/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Handling of Objective-C classes and metaclasses.
//!
//! Note that metaclasses are just a special case of classes.
//!
//! Resources:
//! - [[objc explain]: Classes and metaclasses](http://www.sealiesoftware.com/blog/archive/2009/04/14/objc_explain_Classes_and_metaclasses.html), especially [the PDF diagram](http://www.sealiesoftware.com/blog/class%20diagram.pdf)

use super::{
    id, ivar_list_t, method_list_t, nil, objc_object, AnyHostObject, HostIMP, HostObject, ObjC,
    IMP, SEL,
};
use crate::libc::string::strdup;
use crate::mach_o::MachO;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, Mem, MutPtr, Ptr, SafeRead};
use crate::Environment;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

/// Generic pointer to an Objective-C class or metaclass.
///
/// The name is standard Objective-C.
///
/// Apple's runtime has a `objc_classes` definition similar to `objc_object`.
/// We could do the same thing here, but it doesn't seem worth it, as we can't
/// get the same unidirectional type safety.
pub type Class = id;

/// Our internal representation of a class, e.g. this is where `objc_msgSend`
/// will look up method implementations.
///
/// Note: `superclass` can be `nil`!
pub(super) struct ClassHostObject {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
    pub(super) superclass: Class,
    pub(super) methods: HashMap<SEL, IMP>,
    pub(super) guest_method_signatures: HashMap<SEL, ConstPtr<u8>>,
    /// Maps ivar name to a tuple of an offset (as pointer), an alignment, and an optional type encoding.
    /// (Alignment is used during ivar reconciliation.)
    pub(super) ivars: HashMap<String, (ConstPtr<GuestUSize>, u32, Option<String>)>,
    /// Offset into the allocated memory for the object where the ivars of
    /// instances of this class or metaclass (respectively: normal objects or
    /// classes) should live. This is always >= the value in the superclass.
    pub(super) instance_start: GuestUSize,
    /// Size of the allocated memory for instances of this class or metaclass.
    /// This is always >= the value in the superclass.
    pub(super) instance_size: GuestUSize,
    /// Checks if +initialize has been called yet.
    pub(super) is_initialized: InitializationStatus,
}
impl HostObject for ClassHostObject {}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum InitializationStatus {
    NotInitialized,
    Initializing,
    Initialized,
}

/// Placeholder object for classes and metaclasses referenced by the app that
/// we don't have an implementation for.
///
/// This lets us delay errors about missing implementations until the first
/// time the app actually uses them (e.g. when a message is sent).
pub(super) struct UnimplementedClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for UnimplementedClass {}

/// Substitute object for classes and metaclasses from the guest app that we do
/// not want to support (see [substitute_classes]).
///
/// Messages sent to this class will behave as if messaging [nil].
pub(super) struct FakeClass {
    pub(super) name: String,
    pub(super) is_metaclass: bool,
}
impl HostObject for FakeClass {}

/// The layout of a class in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_t {
    isa: Class, // note that this matches objc_object
    superclass: Class,
    _cache: ConstVoidPtr,
    _vtable: ConstVoidPtr,
    data: ConstPtr<class_rw_t>,
}
unsafe impl SafeRead for class_t {}

/// The layout of the main class data in an app binary.
///
/// The name, field names and field layout are based on what Ghidra's output.
#[repr(C, packed)]
#[allow(dead_code)]
struct class_rw_t {
    _flags: u32,
    instance_start: GuestUSize,
    instance_size: GuestUSize,
    _reserved: u32,
    name: ConstPtr<u8>,
    base_methods: ConstPtr<method_list_t>,
    _base_protocols: ConstVoidPtr, // protocol list (TODO)
    ivars: ConstPtr<ivar_list_t>,
    _weak_ivar_layout: u32,
    _base_properties: ConstVoidPtr, // property list (TODO)
}
unsafe impl SafeRead for class_rw_t {}

/// The layout of a category in an app binary.
///
/// The name, field names and field layout are based on what Ghidra outputs.
#[repr(C, packed)]
struct category_t {
    name: ConstPtr<u8>,
    class: Class,
    instance_methods: ConstPtr<method_list_t>,
    class_methods: ConstPtr<method_list_t>,
    _protocols: ConstVoidPtr,     // protocol list (TODO)
    _property_list: ConstVoidPtr, // property list (TODO)
}
unsafe impl SafeRead for category_t {}

#[repr(C, packed)]
pub struct objc_property {
    // TODO: define fields?
    _pad: u8,
}
unsafe impl SafeRead for objc_property {}

/// An opaque type that represents an Objective-C declared property.
#[allow(non_camel_case_types)]
type objc_property_t = MutPtr<objc_property>;

/// A template for a class defined with [objc_classes].
///
/// Host implementations of libraries can use these to expose classes to the
/// application. The runtime will create the actual class ([ClassHostObject]
/// etc) from the template on-demand. See also [ClassExports].
pub struct ClassTemplate {
    pub name: &'static str,
    pub superclass: Option<&'static str>,
    pub class_methods: &'static [(&'static str, &'static dyn HostIMP)],
    pub instance_methods: &'static [(&'static str, &'static dyn HostIMP)],
}

/// Type for lists of classes exported by host implementations of frameworks.
///
/// Each module that wants to expose functions to guest code should export a
/// constant using this type. See [objc_classes] for an example.
///
/// All the constants like this can then be collected into a
/// [crate::dyld::HostDylib].
///
/// The strings are the class names.
///
/// See also [crate::dyld::ConstantExports] and [crate::dyld::FunctionExports].
pub type ClassExports = &'static [(&'static str, ClassTemplate)];

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_superclass {
    (: $name:ident) => {
        Some(stringify!($name))
    };
    () => {
        None
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _objc_method {
    (
        $env:ident,
        $this:ident,
        $_cmd:ident,
        $cmd_name:ident,
        $retty:ty,
        $block:block
        $(, $ty:ty, $arg:ident)*
        $(, ...$va_arg:ident: $va_type:ty)?
    ) => {
        // The closure must be explicitly casted because a bare closure defaults
        // to a different type than a pure fn pointer, which is the type that
        // HostIMP and CallFromGuest are implemented on.
        &((|
            #[allow(unused_variables)]
            $env: &mut $crate::Environment,
            #[allow(unused_variables)]
            $this: $crate::objc::id,
            #[allow(unused_variables)]
            $_cmd: $crate::objc::SEL,
            $($arg: $ty,)*
            $(#[allow(unused_mut)] mut $va_arg: $va_type,)?
        | -> $retty {
            const _OBJC_CURRENT_SELECTOR: &str = stringify!($cmd_name);
            $block
        }) as fn(
            &mut $crate::Environment,
            $crate::objc::id,
            $crate::objc::SEL,
            $($ty,)*
            $($va_type,)?
        ) -> $retty)
    }
}

/// Macro for creating a list of [ClassTemplate]s (i.e. [ClassExports]).
/// It imitates the Objective-C class definition syntax.
///
/// ```ignore
/// pub const CLASSES: ClassExports = objc_classes! {
/// (env, this, _cmd); // Specify names of HostIMP implicit parameters.
///                    // The second one should be `self` to match Objective-C,
///                    // but that's reserved in Rust, hence `this`.
///
/// @implementation MyClass: NSObject
///
/// + (id)foo {
///     // ...
/// }
///
/// - (id)barWithQux:(u32)qux {
///     // ...
/// }
///
/// - (id)barWithVaArgs:(u32)qux, ...dots {
///     // ...
/// }
///
/// @end
/// };
/// ```
///
/// will desugar to approximately:
///
/// ```ignore
/// pub const CLASSES: ClassExports = &[
///     ("MyClass", ClassTemplate {
///         name: "MyClass",
///         superclass: Some("NSObject"),
///         class_methods: &[
///             ("foo", &(|env: &mut Environment, this: id, _cmd: SEL| -> id {
///                 // ...
///             } as fn(&mut Environment, id, SEL) -> id)),
///         ],
///         instance_methods: &[
///             ("barWithQux:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32) -> id)),
///             ("barWithVaArgs:", &(|
///                 env: &mut Environment,
///                 this: id,
///                 _cmd: SEL,
///                 qux: u32,
///                 va_args: DotDotDot
///             | -> id {
///                 // ...
///             } as &fn(&mut Environment, id, SEL, u32, DotDotDot) -> id)),
///         ],
///     })
/// ];
/// ```
///
/// Note that the instance methods must be preceded by the class methods.
#[macro_export] // documentation comment links are annoying without this
macro_rules! objc_classes {
    {
        // Rust's macro hygiene prevents the macro's own names for these
        // parameters being visible, so we have to get names supplied by the
        // macro user.
        ($env:ident, $this:ident, $_cmd:ident);
        $(
            @implementation $class_name:ident $(: $superclass_name:ident)?

            $( + ($cm_type:ty) $cm_name:ident $(:($cm_type1:ty) $cm_arg1:ident $($($cm_namen:ident)?:($cm_typen:ty) $cm_argn:ident)*)?
                              $(, ...$cm_va_arg:ident)?
                 $cm_block:block )*

            $( - ($im_type:ty) $im_name:ident $(:($im_type1:ty) $im_arg1:ident $($($im_namen:ident)?:($im_typen:ty) $im_argn:ident)*)?
                              $(, ...$im_va_arg:ident)?
                 $im_block:block )*

            @end
        )+
    } => {
        &[
            $({
                // This constant is for `msg_super!`, which needs to know which
                // class it is has been written within (not the same as the
                // runtime type of `this`, which could be a subclass). This is
                // a constant instead of a let binding because that escapes
                // Rust's macro hygiene.
                const _OBJC_CURRENT_CLASS: &str = stringify!($class_name);

                (_OBJC_CURRENT_CLASS, $crate::objc::ClassTemplate {
                    name: _OBJC_CURRENT_CLASS,
                    superclass: $crate::_objc_superclass!($(: $superclass_name)?),
                    class_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($cm_type1);)?
                                    $cm_name
                                    $($(, $($cm_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $cm_name,
                                    $cm_type,
                                    { $cm_block }
                                    $(, $cm_type1, $cm_arg1 $(, $cm_typen, $cm_argn)*)?
                                    $(, ...$cm_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                    instance_methods: &[
                        $(
                            (
                                $crate::objc::selector!(
                                    $(($im_type1);)?
                                    $im_name
                                    $($(, $($im_namen)?)*)?
                                ),
                                $crate::_objc_method!(
                                    $env,
                                    $this,
                                    $_cmd,
                                    $im_name,
                                    $im_type,
                                    { $im_block }
                                    $(, $im_type1, $im_arg1 $(, $im_typen, $im_argn)*)?
                                    $(, ...$im_va_arg: $crate::abi::DotDotDot)?
                                )
                            )
                        ),*
                    ],
                })
            }),+
        ]
    }
}
pub use crate::objc_classes; // #[macro_export] is weird...

impl ClassHostObject {
    fn from_template(
        template: &ClassTemplate,
        is_metaclass: bool,
        superclass: Class,
        objc: &ObjC,
    ) -> Self {
        // For our host implementations we store all data in host objects, so
        // there are no ivars and the size is always just the isa pointer.
        // This is true for both classes and normal objects.
        let size = guest_size_of::<objc_object>();
        ClassHostObject {
            name: template.name.to_string(),
            is_metaclass,
            superclass,
            methods: HashMap::from_iter(
                (if is_metaclass {
                    template.class_methods
                } else {
                    template.instance_methods
                })
                .iter()
                .map(|&(name, host_imp)| {
                    // The selector should already have been registered by
                    // [ObjC::register_host_selectors], so we can panic
                    // if it hasn't been.
                    (objc.selectors[name], IMP::Host(host_imp))
                }),
            ),
            guest_method_signatures: HashMap::default(),
            // maybe this should be 0 for NSObject? does it matter?
            instance_start: size,
            instance_size: size,
            ivars: HashMap::default(),
            is_initialized: InitializationStatus::NotInitialized,
        }
    }

    fn from_bin(class: Class, is_metaclass: bool, mem: &Mem, objc: &mut ObjC) -> Self {
        let class_t {
            superclass, data, ..
        } = mem.read(class.cast());
        let class_rw_t {
            instance_start,
            instance_size,
            name,
            base_methods,
            ivars,
            ..
        } = mem.read(data);

        let name = mem.cstr_at_utf8(name).unwrap().to_string();

        let mut host_object = ClassHostObject {
            name,
            is_metaclass,
            superclass,
            methods: HashMap::new(),
            guest_method_signatures: HashMap::new(),
            instance_start,
            instance_size,
            ivars: HashMap::new(),
            is_initialized: InitializationStatus::NotInitialized,
        };

        if !base_methods.is_null() {
            host_object.add_methods_from_bin(base_methods, mem, objc);
        }

        if !ivars.is_null() {
            host_object.add_ivars_from_bin(ivars, mem);
        }

        host_object
    }

    // See methods.rs for binary method parsing
}

/// Decide whether a certain class/metaclass pair from the guest app should use
/// fake class host objects and return the substitutions if so.
///
/// This function is called when registering classes from the guest app. It
/// detects certain problematic classes that are, for example, too complex for
/// touchHLE to currently support, but which can be easily replaced with simple
/// fakes.
fn substitute_classes(
    mem: &Mem,
    class: Class,
    metaclass: Class,
) -> Option<(Box<FakeClass>, Box<FakeClass>)> {
    let class_t { data, .. } = mem.read(class.cast());
    let class_rw_t { name, .. } = mem.read(data);
    let name = mem.cstr_at_utf8(name).unwrap();

    // Currently the only thing we try to substitute: classes that seem to be
    // from various third-party advertising or social network SDKs.
    // Naturally it makes a lot of use of UIKit and networking in ways we
    // don't support yet. This isn't "ad blocking" because ads no longer work
    // on real devices anyway :)
    if !(name.starts_with("AdMob")
        || name.starts_with("AdWhirl")
        || name.starts_with("AltAds")
        || name.starts_with("FB") // Facebook
        || name.starts_with("Flurry")
        || name.starts_with("Mobclix")
        || name.starts_with("OpenFeint")
        || name.starts_with("Tapjoy"))
    {
        return None;
    }

    {
        let class_t { data, .. } = mem.read(metaclass.cast());
        let class_rw_t {
            name: metaclass_name,
            ..
        } = mem.read(data);
        let metaclass_name = mem.cstr_at_utf8(metaclass_name).unwrap();
        assert!(name == metaclass_name);
    }

    log!(
        "Note: substituting fake class for {} to improve compatibility",
        name
    );

    let class_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: false,
    });
    let metaclass_host_object = Box::new(FakeClass {
        name: name.to_string(),
        is_metaclass: true,
    });
    Some((class_host_object, metaclass_host_object))
}

impl ObjC {
    /// Iterator over all known classes and their names.
    pub fn all_classes(&self) -> impl Iterator<Item = (&String, &Class)> {
        self.classes.iter()
    }

    fn get_class(&self, name: &str, is_metaclass: bool, mem: &Mem) -> Option<Class> {
        let class = self.classes.get(name).copied()?;
        Some(if is_metaclass {
            Self::read_isa(class, mem)
        } else {
            class
        })
    }

    fn find_template(name: &str) -> Option<&'static ClassTemplate> {
        crate::dyld::search_host_dylibs(|dylib| dylib.class_exports, name)
            .map(|&(_name, ref template)| template)
    }

    /// For use by [crate::dyld]: get the class or metaclass referenced by an
    /// external relocation in the app binary. If we don't have an
    /// implementation of the class, a placeholder is used.
    pub fn link_class(&mut self, name: &str, is_metaclass: bool, mem: &mut Mem) -> Class {
        self.link_class_inner(name, is_metaclass, mem, true)
    }

    /// For use by host functions: get a particular class. If we don't have an
    /// implementation of the class, panic.
    pub fn get_known_class(&mut self, name: &str, mem: &mut Mem) -> Class {
        self.link_class_inner(name, /* is_metaclass: */ false, mem, false)
    }

    /// Try to find a class either already registered or with a host template, without falling back to a placeholder.
    pub fn find_class(&mut self, name: &str, mem: &mut Mem) -> Option<Class> {
        if self.classes.contains_key(name) || Self::find_template(name).is_some() {
            Some(self.link_class(name, false, mem))
        } else {
            None
        }
    }

    fn link_class_inner(
        &mut self,
        name: &str,
        is_metaclass: bool,
        mem: &mut Mem,
        use_placeholder: bool,
    ) -> Class {
        // The class and metaclass must be created together and tracked
        // together, so even though this function only returns one pointer, it
        // must create both. The function must not care whether the metaclass
        // is requested first, or if the class is requested first.

        if let Some(class) = self.get_class(name, is_metaclass, mem) {
            return class;
        };

        let class_host_object: Box<dyn AnyHostObject>;
        let metaclass_host_object: Box<dyn AnyHostObject>;
        if let Some(template) = Self::find_template(name) {
            // We have a template (host implementation) for this class, use it.

            if let Some(superclass_name) = template.superclass {
                // Make sure we actually have a template for the superclass
                // before we try to link it, else we might get an unimplemented
                // class back and have weird problems down the line
                assert!(Self::find_template(superclass_name).is_some());
            }

            class_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ false,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ false, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
            metaclass_host_object = Box::new(ClassHostObject::from_template(
                template,
                /* is_metaclass: */ true,
                /* superclass: */
                template
                    .superclass
                    .map(|name| {
                        self.link_class(name, /* is_metaclass: */ true, mem)
                    })
                    .unwrap_or(nil),
                self,
            ));
        } else {
            if !use_placeholder {
                panic!("Missing implementation for class {name}!");
            }

            // We don't have a real implementation for this class, use a
            // placeholder.

            class_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: false,
            });
            metaclass_host_object = Box::new(UnimplementedClass {
                name: name.to_string(),
                is_metaclass: true,
            });
        }

        // NSObject's metaclass is special: it is its own metaclass, and it's
        // the superclass of all other metaclasses.
        // (FIXME: this actually should apply to any class hiearchy root.)
        // This creates a chicken-and-egg problem so it has a special path.
        let metaclass = if name == "NSObject" {
            let metaclass = mem.alloc_and_write(objc_object { isa: nil });
            mem.write(metaclass, objc_object { isa: metaclass });
            self.register_static_object(metaclass, metaclass_host_object);
            metaclass
        } else {
            let isa = self.link_class("NSObject", /* is_metaclass: */ true, mem);
            self.alloc_static_object(isa, metaclass_host_object, mem)
        };

        let class = self.alloc_static_object(metaclass, class_host_object, mem);

        if name == "NSObject" {
            // NSObject's metaclass has its class as the superclass.
            self.borrow_mut::<ClassHostObject>(metaclass).superclass = class;
        }

        self.classes.insert(name.to_string(), class);

        if is_metaclass {
            metaclass
        } else {
            class
        }
    }

    /// For use by [crate::dyld]: register all the classes from the application
    /// binary.
    pub fn register_bin_classes(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_classlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<Class> = Ptr::from_bits(list.addr);
        for i in 0..(list.size / 4) {
            let class = mem.read(base + i);
            let metaclass = Self::read_isa(class, mem);

            let name = if let Some(fakes) = substitute_classes(mem, class, metaclass) {
                let (class_host_object, metaclass_host_object) = fakes;

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            } else {
                let class_host_object = Box::new(ClassHostObject::from_bin(
                    class, /* is_metaclass: */ false, mem, self,
                ));
                let metaclass_host_object = Box::new(ClassHostObject::from_bin(
                    metaclass, /* is_metaclass: */ true, mem, self,
                ));

                assert!(class_host_object.name == metaclass_host_object.name);
                let name = class_host_object.name.clone();

                self.register_static_object(class, class_host_object);
                self.register_static_object(metaclass, metaclass_host_object);
                name
            };

            self.classes.insert(name.to_string(), class);
        }

        let mut queue = VecDeque::<Class>::new();
        let mut found_ns_object = false;
        // Second pass to build an inverted inheritance graph
        let mut inverted_inheritance = HashMap::<Class, Vec<Class>>::new();
        for (name, class) in self.classes.iter() {
            log_dbg!("class name {}", name);
            if name == "NSObject" {
                assert!(!found_ns_object);
                found_ns_object = true;
            }
            let class_host_object = self
                .get_host_object(*class)
                .unwrap()
                .as_any()
                .downcast_ref();
            let Some(ClassHostObject { superclass, .. }) = class_host_object else {
                // Skip FakeClass or UnimplementedClass
                continue;
            };

            if *superclass != nil {
                inverted_inheritance
                    .entry(*superclass)
                    .and_modify(|v| v.push(*class))
                    .or_insert(vec![*class]);
            } else {
                assert!(!queue.contains(class));
                queue.push_back(*class);
            }
        }
        // At least NSObject should be found as a root object
        assert!(found_ns_object);

        // Third pass to ensure no superclass has "grown into" any of its
        // subclasses.
        // (https://alwaysprocessing.blog/2023/03/12/objc-ivar-abi)

        // BFS starting from root(s) for ivar reconciliation
        //
        // It is required to be traversed in this order,
        // as one overgrown class potentially implies
        // reconciliation for _all of subclasses_
        while !queue.is_empty() {
            let next = queue.pop_front().unwrap();
            self.reconcile_ivars_for_class(next, mem);
            if let Some(subclasses) = inverted_inheritance.get(&next) {
                queue.extend(subclasses);
            }
        }
    }

    pub fn reconcile_ivars_for_class(&mut self, class: Class, mem: &mut Mem) {
        let (need, mut diff) = self.need_ivar_reconciliation(class);
        if need {
            let ClassHostObject {
                name, superclass, ..
            } = self.borrow(class);
            log_dbg!(
                "Class {} need ivar reconciliation with superclass {}!",
                name,
                &self.borrow::<ClassHostObject>(*superclass).name
            );

            let ClassHostObject {
                ref mut instance_start,
                ref mut instance_size,
                ref ivars,
                ..
            } = self.borrow_mut(class);

            if !ivars.is_empty() {
                let mut max_alignment: u32 = 1;
                for (offset, align, _) in ivars.values() {
                    if offset.is_null() {
                        // anonymous bitfield
                        continue;
                    }
                    max_alignment = max_alignment.max(*align);
                }

                if max_alignment > 1 {
                    let max_alignment = max_alignment.next_power_of_two();
                    let align_mask = max_alignment - 1;
                    diff = (diff + align_mask) & !align_mask;
                }

                for (offset, _, _) in ivars.values() {
                    if offset.is_null() {
                        // anonymous bitfield
                        continue;
                    }

                    let current_offset: GuestUSize = mem.read(*offset);
                    mem.write(offset.cast_mut(), current_offset + diff);
                }
            }

            *instance_start += diff;
            *instance_size += diff;
        }
    }

    pub fn need_ivar_reconciliation(&mut self, class: Class) -> (bool, u32) {
        let class_host_object = self.get_host_object(class).unwrap().as_any().downcast_ref();
        let Some(ClassHostObject {
            name,
            superclass,
            instance_start,
            instance_size,
            ..
        }) = class_host_object
        else {
            // The class might be a FakeClass or UnimplementedClass
            // In those cases we move on as they don't have ivars
            return (false, 0);
        };
        log_dbg!(
            "Checking need_ivar_reconciliation for {}, start {}, size {}",
            name,
            instance_start,
            instance_size
        );

        if *superclass == nil {
            return (false, 0);
        }

        let superclass_host_object = self
            .get_host_object(*superclass)
            .unwrap()
            .as_any()
            .downcast_ref();
        let Some(ClassHostObject {
            instance_size: superclass_instance_size,
            ..
        }) = superclass_host_object
        else {
            // Superclass could also be a FakeClass or UnimplementedClass
            return (false, 0);
        };

        let need = instance_start < superclass_instance_size;
        let diff = if need {
            superclass_instance_size - instance_start
        } else {
            0
        };
        (need, diff)
    }

    /// Dumps all classes available to the emulator in JSON to stdout.
    ///
    /// The JSON has the following form:
    /// ```json
    /// {
    ///     "object": "classes",
    ///     "classes": [
    ///         {
    ///             "name": ((name of class)),
    ///             "super": ((name of superclass, if available)),
    ///             "class_type": (("normal" | "unimplemented" | "fake"))
    ///         },
    ///         ...
    ///     ]
    /// }
    /// ```
    pub fn dump_classes(&self, file: &mut std::fs::File) -> Result<(), std::io::Error> {
        use std::io::Write;
        writeln!(file, "{{\n    \"object\": \"classes\",\n    \"classes\": [")?;
        for (i, (_, o)) in self.classes.iter().enumerate() {
            // Why doesn't json allow trailing commas...
            let comma = if i == self.classes.len() - 1 { "" } else { "," };

            let host_obj = self.get_host_object(*o).unwrap();

            if let Some(ClassHostObject {
                name,
                superclass: sup,
                ..
            }) = host_obj.as_any().downcast_ref()
            {
                if *sup == nil {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{name}\", \"class_type\": \"normal\" }}{comma}"
                    )?;
                } else {
                    writeln!(
                        file,
                        "        {{ \"name\": \"{}\", \"super\": \"{}\", \"class_type\": \"normal\" }}{}",
                        name, self.get_class_name(*sup), comma
                    )?;
                }
            } else if let Some(UnimplementedClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"unimplemented\" }}{comma}"
                )?;
            } else if let Some(FakeClass { name, .. }) = host_obj.as_any().downcast_ref() {
                writeln!(
                    file,
                    "        {{ \"name\": \"{name}\", \"class_type\": \"fake\" }}{comma}"
                )?;
            } else {
                panic!("Unrecognized class type!");
            }
        }
        writeln!(file, "    ]\n}}")
    }

    /// For use by [crate::dyld]: register all the categories from the
    /// application binary.
    pub fn register_bin_categories(&mut self, bin: &MachO, mem: &mut Mem) {
        let Some(list) = bin.get_section("__objc_catlist") else {
            return;
        };

        assert!(list.size % 4 == 0);
        let base: ConstPtr<ConstPtr<category_t>> = Ptr::from_bits(list.addr);
        for i in 0..(list.size / 4) {
            let cat_ptr = mem.read(base + i);
            let data = mem.read(cat_ptr);

            let name = mem.cstr_at_utf8(data.name).unwrap();
            let class = data.class;
            let metaclass = Self::read_isa(class, mem);

            for (class, methods) in [
                (class, data.instance_methods),
                (metaclass, data.class_methods),
            ] {
                if methods.is_null() {
                    continue;
                }

                let any = self.get_host_object(class).unwrap().as_any();
                if any.is::<FakeClass>() || any.is::<UnimplementedClass>() {
                    continue;
                }

                // Horrible workaround to avoid double-borrowing self:
                // temporarily replace the class object.
                let mut host_obj = std::mem::replace(
                    self.borrow_mut::<ClassHostObject>(class),
                    ClassHostObject {
                        name: Default::default(),
                        is_metaclass: Default::default(),
                        superclass: nil,
                        methods: Default::default(),
                        guest_method_signatures: Default::default(),
                        instance_start: Default::default(),
                        instance_size: Default::default(),
                        ivars: Default::default(),
                        is_initialized: InitializationStatus::NotInitialized,
                    },
                );
                log_dbg!(
                    "Adding {} methods from guest app category \"{}\" {:?} to {} \"{}\" {:?}",
                    if host_obj.is_metaclass {
                        "class"
                    } else {
                        "instance"
                    },
                    name,
                    cat_ptr,
                    if host_obj.is_metaclass {
                        "metaclass"
                    } else {
                        "class"
                    },
                    host_obj.name,
                    class,
                );
                host_obj.add_methods_from_bin(methods, mem, self);
                *self.borrow_mut::<ClassHostObject>(class) = host_obj;
            }
        }
    }

    pub fn class_is_subclass_of(&self, class: Class, superclass: Class) -> bool {
        if class == superclass {
            return true;
        }

        let mut class = class;
        loop {
            let &ClassHostObject {
                superclass: next, ..
            } = self.borrow(class);
            if next == nil {
                return false;
            } else if next == superclass {
                return true;
            } else {
                class = next;
            }
        }
    }

    pub fn get_class_name(&self, class: Class) -> &str {
        self.try_get_class_name(class)
            .expect("Could not get class name!")
    }

    pub fn get_superclass(&self, class: Class) -> Class {
        let &ClassHostObject { superclass, .. } = self.borrow(class);
        superclass
    }

    pub fn try_get_class_name(&self, class: Class) -> Option<&str> {
        let host_object = self.get_host_object(class)?;
        if let Some(ClassHostObject { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(UnimplementedClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else if let Some(FakeClass { name, .. }) = host_object.as_any().downcast_ref() {
            Some(name)
        } else {
            None
        }
    }

    pub fn is_unimplemented_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(
            host_object.as_any().downcast_ref(),
            Some(UnimplementedClass { .. })
        )
    }

    pub fn is_fake_class(&self, class: Class) -> bool {
        if class == nil {
            return false;
        }
        let host_object = self.get_host_object(class).unwrap();
        matches!(host_object.as_any().downcast_ref(), Some(FakeClass { .. }))
    }
}

pub(super) fn objc_getClass(env: &mut Environment, name: ConstPtr<u8>) -> id {
    let name_str = env.mem.cstr_at_utf8(name).unwrap();
    env.objc
        .get_class(name_str, false, &env.mem)
        .unwrap_or_else(|| panic!("objc_getClass() for unimplemented class {name_str}"))
}

pub(super) fn class_getSuperclass(env: &mut Environment, cls: Class) -> Class {
    if cls == nil {
        nil
    } else {
        env.objc.borrow::<ClassHostObject>(cls).superclass
    }
}

pub(super) fn class_getInstanceSize(env: &mut Environment, cls: Class) -> GuestUSize {
    if cls == nil {
        0
    } else {
        env.objc.borrow::<ClassHostObject>(cls).instance_size
    }
}

pub(super) fn class_getProperty(
    env: &mut Environment,
    cls: Class,
    name: ConstPtr<u8>,
) -> objc_property_t {
    if cls == nil {
        return Ptr::null();
    }
    let c_name = env.mem.cstr_at_utf8(name).unwrap();
    let class_name_string = env.objc.get_class_name(cls).to_owned();
    if class_name_string == "UIScreen" && c_name == "scale" {
        // Even if [UIScreen scale] is implemented, we're not yet having a
        // proper support for `objc_property_t`, so we prefer to return a NULL
        // here (e.g. property is not declared).
        // Some games (such as Mirror's Edge) check for those to conditionally
        // apply some parameters depending on the iOS version without actually
        // using the property.
        // We also prefer to not define this as a game-specific hack, because
        // some other EA games may rely on the same logic.
        // TODO: support `objc_property_t` properly
        log!("TODO: class_getProperty(UIScreen, scale) -> NULL");
        return Ptr::null();
    }
    todo!()
}

pub(super) fn class_replaceMethod(
    env: &mut Environment,
    cls: Class,
    name: SEL,
    imp: IMP,
    types: ConstPtr<u8>,
) -> IMP {
    // TODO: avoid unnecessary copy of types
    let types_copy = strdup(env, types).cast_const();
    let &mut ClassHostObject {
        ref mut methods,
        ref mut guest_method_signatures,
        ..
    } = env.objc.borrow_mut(cls);
    if let Entry::Vacant(e) = methods.entry(name) {
        // TODO: use `class_addMethod` once implemented
        log_dbg!(
            "class_replaceMethod: adding new implementation {:?} for method {}",
            imp,
            name.as_str(&env.mem)
        );
        e.insert(imp);
        guest_method_signatures.insert(name, types_copy);
        return IMP::guest_null();
    }
    env.mem.free(types_copy.cast().cast_mut());
    // TODO: use `method_setImplementation` once implemented
    // Note: encoding types are ignored
    let existing = methods.insert(name, imp.clone()).unwrap();
    assert!(matches!(existing, IMP::Guest(_))); // TODO
    log_dbg!(
        "class_replaceMethod: existing {:?} replaced with {:?} for method {}",
        existing,
        imp,
        name.as_str(&env.mem)
    );
    existing
}

pub fn class_getMethodImplementation(env: &mut Environment, cls: Class, name: SEL) -> IMP {
    if cls == nil {
        return IMP::guest_null();
    }
    let mut class = cls;
    loop {
        let &ClassHostObject {
            superclass: next,
            ref methods,
            ..
        } = env.objc.borrow(class);
        if let Some(method) = methods.get(&name) {
            match method {
                IMP::Guest(guest_imp) => return IMP::Guest(*guest_imp),
                IMP::Host(_) => {
                    log_dbg!(
                        "class_getMethodImplementation: returning null for host method {:?}",
                        name.as_str(&env.mem)
                    );
                    return IMP::guest_null();
                }
            }
        } else if next == nil {
            // Unimplemented method
            return IMP::guest_null();
        }
        class = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_class(
        objc: &mut ObjC,
        mem: &mut Mem,
        name: &str,
        superclass: Class,
        instance_start: GuestUSize,
        instance_size: GuestUSize,
        ivars: Vec<(&str, GuestUSize, u32)>, // name, initial offset, alignment
    ) -> (Class, HashMap<String, ConstPtr<GuestUSize>>) {
        let mut cls_ptr: Class = mem.alloc(guest_size_of::<objc_object>()).cast();
        if cls_ptr.is_null() {
            cls_ptr = mem.alloc(guest_size_of::<objc_object>()).cast();
        }
        assert!(!cls_ptr.is_null());
        mem.write(cls_ptr, objc_object { isa: cls_ptr });

        let mut ivar_map = HashMap::new();
        let mut offset_ptrs = HashMap::new();
        for (ivar_name, initial_offset, align) in ivars {
            let offset_ptr: MutPtr<GuestUSize> = mem.alloc(4).cast();
            assert!(!offset_ptr.is_null());
            mem.write(offset_ptr, initial_offset);
            let const_ptr = offset_ptr.cast_const();
            ivar_map.insert(ivar_name.to_string(), (const_ptr, align, None));
            offset_ptrs.insert(ivar_name.to_string(), const_ptr);
        }

        let host_obj = Box::new(ClassHostObject {
            name: name.to_string(),
            is_metaclass: false,
            superclass,
            methods: HashMap::new(),
            guest_method_signatures: HashMap::new(),
            instance_start,
            instance_size,
            ivars: ivar_map,
            is_initialized: InitializationStatus::NotInitialized,
        });

        objc.register_static_object(cls_ptr, host_obj);
        objc.classes.insert(name.to_string(), cls_ptr);
        (cls_ptr, offset_ptrs)
    }

    #[test]
    fn test_ivar_reconciliation_slides_guest_memory_not_host_pointer() {
        let mut mem = Mem::new();
        let mut objc = ObjC::new();

        let (super_cls, _super_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "SuperClass",
            nil,
            4,
            12,
            vec![("ivarA", 4, 4), ("ivarB", 8, 4)],
        );

        let (sub_cls, sub_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "SubClass",
            super_cls,
            8,
            16,
            vec![("subIvar1", 8, 4), ("subIvar2", 12, 4)],
        );

        let orig_x_ptr = sub_ptrs["subIvar1"];
        let orig_y_ptr = sub_ptrs["subIvar2"];

        assert_eq!(mem.read(orig_x_ptr), 8);
        assert_eq!(mem.read(orig_y_ptr), 12);

        let (need, diff) = objc.need_ivar_reconciliation(sub_cls);
        assert_eq!(need, true);
        assert_eq!(diff, 4);

        objc.reconcile_ivars_for_class(sub_cls, &mut mem);

        let host_obj = objc.borrow::<ClassHostObject>(sub_cls);
        assert_eq!(host_obj.instance_start, 12);
        assert_eq!(host_obj.instance_size, 20);

        // Host pointers in ClassHostObject.ivars must NOT change
        assert_eq!(host_obj.ivars["subIvar1"].0, orig_x_ptr);
        assert_eq!(host_obj.ivars["subIvar2"].0, orig_y_ptr);

        // Guest memory AT the pointers must have slid by diff=4
        assert_eq!(mem.read(orig_x_ptr), 12);
        assert_eq!(mem.read(orig_y_ptr), 16);

        // Verify object_lookup_ivar retrieves non-overlapping addresses
        let instance: id = mem.alloc(20).cast();
        mem.write(instance, objc_object { isa: sub_cls });

        let (ptr_a, _) = objc.object_lookup_ivar(&mem, instance, "ivarA").unwrap();
        let (ptr_b, _) = objc.object_lookup_ivar(&mem, instance, "ivarB").unwrap();
        let (ptr_1, _) = objc.object_lookup_ivar(&mem, instance, "subIvar1").unwrap();
        let (ptr_2, _) = objc.object_lookup_ivar(&mem, instance, "subIvar2").unwrap();

        assert_eq!(ptr_a.to_bits(), instance.to_bits() + 4);
        assert_eq!(ptr_b.to_bits(), instance.to_bits() + 8);
        assert_eq!(ptr_1.to_bits(), instance.to_bits() + 12);
        assert_eq!(ptr_2.to_bits(), instance.to_bits() + 16);
    }

    #[test]
    fn test_maplestory_status_panel_passive_skills_pane_skill_cell_layout() {
        let mut mem = Mem::new();
        let mut objc = ObjC::new();

        // 1. UIViewController (base)
        let (uiviewcontroller_cls, _) = create_test_class(
            &mut objc,
            &mut mem,
            "UIViewController",
            nil,
            4,
            0x98,
            vec![],
        );

        // 2. InventoryViewController: superclass UIViewController, size 0xa4
        let (inv_view_cls, inv_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "InventoryViewController",
            uiviewcontroller_cls,
            0x98,
            0xa4,
            vec![
                ("itemCell", 0x98, 4),
                ("equipmentCell", 0x9c, 4),
                ("skillCell", 0xa0, 4),
            ],
        );

        // 3. PassiveSkillsPane: superclass InventoryViewController
        // Compiled with instance_start=0x98, instance_size=0xb0
        let (passive_cls, passive_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "PassiveSkillsPane",
            inv_view_cls,
            0x98,
            0xb0,
            vec![
                ("panel", 0x98, 4),
                ("skillListView", 0x9c, 4),
                ("allSkillNames", 0xa0, 4),
                ("selectedCell", 0xa4, 4),
                ("selectedSkill", 0xa8, 4),
                ("confirmPanel", 0xac, 4),
            ],
        );

        // 4. ActiveSkillsPane: superclass InventoryViewController
        let (active_cls, active_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "ActiveSkillsPane",
            inv_view_cls,
            0x98,
            0xa4,
            vec![
                ("panel", 0x98, 4),
                ("selectedSkillView", 0x9c, 4),
                ("skillListView", 0xa0, 4),
            ],
        );

        // 5. StatusPane: superclass InventoryViewController
        let (status_pane_cls, status_pane_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "StatusPane",
            inv_view_cls,
            0x98,
            0xd0,
            vec![
                ("panel", 0x98, 4),
                ("statusPaneBackgournd", 0x9c, 4),
                ("portraitView", 0xa0, 4),
            ],
        );

        // 6. InventoryCell (superclass of SkillCell)
        let (inv_cell_cls, inv_cell_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "InventoryCell",
            nil,
            0xf4,
            0x104,
            vec![
                ("displayName", 0xf4, 4),
                ("iconView", 0xf8, 4),
                ("panel", 0xfc, 4),
                ("dimmerView_", 0x100, 4),
            ],
        );

        // 7. SkillCell: superclass InventoryCell
        let (skill_cell_cls, skill_cell_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "SkillCell",
            inv_cell_cls,
            0x100,
            0x108,
            vec![
                ("descriptionView", 0x100, 4),
                ("useButton", 0x104, 4),
            ],
        );

        // Reconcile PassiveSkillsPane
        objc.reconcile_ivars_for_class(passive_cls, &mut mem);
        assert_eq!(mem.read(passive_ptrs["panel"]), 0xa4);
        assert_eq!(mem.read(passive_ptrs["skillListView"]), 0xa8);
        assert_eq!(mem.read(passive_ptrs["allSkillNames"]), 0xac);
        assert_eq!(mem.read(passive_ptrs["selectedCell"]), 0xb0);
        assert_eq!(mem.read(passive_ptrs["selectedSkill"]), 0xb4);
        assert_eq!(mem.read(passive_ptrs["confirmPanel"]), 0xb8);

        // Crucial verification: allSkillNames (0xac) must NOT collide with skillCell (0xa0)
        let skill_cell_offset = mem.read(inv_ptrs["skillCell"]);
        let all_skill_names_offset = mem.read(passive_ptrs["allSkillNames"]);
        assert_eq!(skill_cell_offset, 0xa0);
        assert_eq!(all_skill_names_offset, 0xac);
        assert_ne!(skill_cell_offset, all_skill_names_offset, "allSkillNames and skillCell must not collide");

        // Reconcile ActiveSkillsPane
        objc.reconcile_ivars_for_class(active_cls, &mut mem);
        assert_eq!(mem.read(active_ptrs["panel"]), 0xa4);
        assert_eq!(mem.read(active_ptrs["selectedSkillView"]), 0xa8);
        assert_eq!(mem.read(active_ptrs["skillListView"]), 0xac);
        assert_ne!(mem.read(active_ptrs["skillListView"]), skill_cell_offset, "ActiveSkillsPane.skillListView must not collide with skillCell");

        // Reconcile StatusPane
        objc.reconcile_ivars_for_class(status_pane_cls, &mut mem);
        assert_eq!(mem.read(status_pane_ptrs["portraitView"]), 0xac);
        assert_ne!(mem.read(status_pane_ptrs["portraitView"]), skill_cell_offset, "StatusPane.portraitView must not collide with skillCell");

        // Reconcile SkillCell
        objc.reconcile_ivars_for_class(skill_cell_cls, &mut mem);
        let dimmer_view_offset = mem.read(inv_cell_ptrs["dimmerView_"]);
        let description_view_offset = mem.read(skill_cell_ptrs["descriptionView"]);
        let use_button_offset = mem.read(skill_cell_ptrs["useButton"]);
        assert_eq!(dimmer_view_offset, 0x100);
        assert_eq!(description_view_offset, 0x104);
        assert_eq!(use_button_offset, 0x108);
        assert_ne!(description_view_offset, dimmer_view_offset, "SkillCell.descriptionView must not collide with dimmerView_");

        // 8. StatusPanel: superclass UIViewController
        // Compiled with instance_start=0x98, instance_size=0xc0
        let (status_panel_cls, status_panel_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "StatusPanel",
            uiviewcontroller_cls,
            0x98,
            0xc0,
            vec![
                ("statusPane", 0x98, 4),
                ("activeSkillsPane", 0x9c, 4),
                ("passiveSkillsPane", 0xa0, 4),
                ("backButton", 0xa4, 4),
                ("goldLabel", 0xa8, 4),
                ("tabButtons", 0xac, 4),
                ("selectedTab", 0xb0, 4),
                ("panes", 0xb4, 4),
                ("currentPane", 0xb8, 4),
                ("panelMode", 0xbc, 4),
            ],
        );

        // Reconcile StatusPanel
        objc.reconcile_ivars_for_class(status_panel_cls, &mut mem);
        assert_eq!(mem.read(status_panel_ptrs["statusPane"]), 0x98);
        assert_eq!(mem.read(status_panel_ptrs["activeSkillsPane"]), 0x9c);
        assert_eq!(mem.read(status_panel_ptrs["passiveSkillsPane"]), 0xa0);
        assert_eq!(mem.read(status_panel_ptrs["backButton"]), 0xa4);
        assert_eq!(mem.read(status_panel_ptrs["goldLabel"]), 0xa8);
        assert_eq!(mem.read(status_panel_ptrs["tabButtons"]), 0xac);
        assert_eq!(mem.read(status_panel_ptrs["selectedTab"]), 0xb0);
        assert_eq!(mem.read(status_panel_ptrs["panes"]), 0xb4);
        assert_eq!(mem.read(status_panel_ptrs["currentPane"]), 0xb8);
        assert_eq!(mem.read(status_panel_ptrs["panelMode"]), 0xbc);

        // Test object_lookup_ivar on an instance of StatusPanel
        let status_panel_inst: id = mem.alloc(0xc0).cast();
        mem.write(status_panel_inst, objc_object { isa: status_panel_cls });

        let (p_back_button, _) = objc.object_lookup_ivar(&mem, status_panel_inst, "backButton").unwrap();
        let (p_passive_skills_pane, _) = objc.object_lookup_ivar(&mem, status_panel_inst, "passiveSkillsPane").unwrap();
        let (p_panel_mode, _) = objc.object_lookup_ivar(&mem, status_panel_inst, "panelMode").unwrap();
        assert_eq!(p_back_button.to_bits(), status_panel_inst.to_bits() + 0xa4);
        assert_eq!(p_passive_skills_pane.to_bits(), status_panel_inst.to_bits() + 0xa0);
        assert_eq!(p_panel_mode.to_bits(), status_panel_inst.to_bits() + 0xbc);

        // Verify sliding of StatusPanel if UIViewController grows (e.g. from 0x98 to 0xa0, diff = 8)
        let (grown_uivc_cls, _) = create_test_class(
            &mut objc,
            &mut mem,
            "GrownUIViewController",
            nil,
            4,
            0xa0,
            vec![],
        );
        let (sliding_status_panel_cls, sliding_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "SlidingStatusPanel",
            grown_uivc_cls,
            0x98,
            0xc0,
            vec![
                ("statusPane", 0x98, 4),
                ("backButton", 0xa4, 4),
                ("panelMode", 0xbc, 4),
            ],
        );
        objc.reconcile_ivars_for_class(sliding_status_panel_cls, &mut mem);
        assert_eq!(mem.read(sliding_ptrs["statusPane"]), 0xa0);
        assert_eq!(mem.read(sliding_ptrs["backButton"]), 0xac);
        assert_eq!(mem.read(sliding_ptrs["panelMode"]), 0xc4);
    }

    #[test]
    fn test_multi_level_ivar_sliding() {
        let mut mem = Mem::new();
        let mut objc = ObjC::new();

        let (root_cls, _) = create_test_class(
            &mut objc,
            &mut mem,
            "Root",
            nil,
            4,
            8,
            vec![("root_ivar", 4, 4)],
        );

        let (mid_cls, mid_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "Mid",
            root_cls,
            4,
            12,
            vec![("mid_ivar", 4, 4)],
        );

        let (leaf_cls, leaf_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "Leaf",
            mid_cls,
            8,
            16,
            vec![("leaf_ivar", 8, 4)],
        );

        // Slide Mid against Root: diff = 8 - 4 = 4
        objc.reconcile_ivars_for_class(mid_cls, &mut mem);
        assert_eq!(mem.read(mid_ptrs["mid_ivar"]), 8);
        assert_eq!(objc.borrow::<ClassHostObject>(mid_cls).instance_size, 16);

        // Slide Leaf against Mid: Leaf was compiled with start 8, but Mid size is now 16!
        // diff = 16 - 8 = 8
        objc.reconcile_ivars_for_class(leaf_cls, &mut mem);
        assert_eq!(mem.read(leaf_ptrs["leaf_ivar"]), 16);
        assert_eq!(objc.borrow::<ClassHostObject>(leaf_cls).instance_size, 24);
    }

    #[test]
    fn test_ivar_reconciliation_alignment() {
        let mut mem = Mem::new();
        let mut objc = ObjC::new();

        let (super_cls, _) = create_test_class(
            &mut objc,
            &mut mem,
            "SuperAlign",
            nil,
            4,
            9, // odd size, diff = 9 - 8 = 1
            vec![],
        );

        let (sub_cls, sub_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "SubAlign",
            super_cls,
            8,
            16,
            vec![("aligned_ivar", 8, 4)], // alignment 4 requires diff to align to 4
        );

        objc.reconcile_ivars_for_class(sub_cls, &mut mem);

        // Diff 1 aligned to mask 3 -> 4
        // Slid offset = 8 + 4 = 12
        assert_eq!(mem.read(sub_ptrs["aligned_ivar"]), 12);
        assert_eq!(objc.borrow::<ClassHostObject>(sub_cls).instance_start, 12);
        assert_eq!(objc.borrow::<ClassHostObject>(sub_cls).instance_size, 20);
    }

    #[test]
    fn test_maplestory_battle_scene_odd_size_alignment() {
        let mut mem = Mem::new();
        let mut objc = ObjC::new();

        // MDBattleScene in MapleStory has odd size 0xf9 (249)
        let (battle_scene_cls, _) = create_test_class(
            &mut objc,
            &mut mem,
            "MDBattleScene",
            nil,
            4,
            0xf9,
            vec![],
        );

        // MDNBattleScene was compiled with instance_start=0xf4 (244) and has 4-byte aligned ivars
        // Raw diff = 0xf9 - 0xf4 = 5.
        // Because ivars have 4-byte alignment, diff must align from 5 to 8, NOT to 6!
        let (nbattle_scene_cls, nbattle_ptrs) = create_test_class(
            &mut objc,
            &mut mem,
            "MDNBattleScene",
            battle_scene_cls,
            0xf4,
            0x10c,
            vec![
                ("peMonsterCharge", 0xf4, 4),
                ("peMonsterHeal", 0xf8, 4),
                ("isSecondAttack", 0xfc, 1),
                ("currentAttack", 0x100, 4),
            ],
        );

        objc.reconcile_ivars_for_class(nbattle_scene_cls, &mut mem);

        // Diff 5 aligned to 4 bytes => 8!
        // 0xf4 + 8 = 0xfc (252, which is 4-byte aligned)
        assert_eq!(mem.read(nbattle_ptrs["peMonsterCharge"]), 0xfc);
        assert_eq!(mem.read(nbattle_ptrs["peMonsterCharge"]) % 4, 0, "peMonsterCharge must be 4-byte aligned");
        assert_eq!(mem.read(nbattle_ptrs["peMonsterHeal"]), 0x100);
        assert_eq!(mem.read(nbattle_ptrs["currentAttack"]), 0x108);
        assert_eq!(objc.borrow::<ClassHostObject>(nbattle_scene_cls).instance_start, 0xfc);
    }
}
