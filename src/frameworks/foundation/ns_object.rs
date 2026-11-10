/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSObject`, the root of most class hierarchies in Objective-C.
//!
//! Resources:
//! - Apple's [Advanced Memory Management Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/MemoryMgmt.html)
//!   explains how reference counting works. Note that we are interested in what
//!   it calls "manual retain-release", not ARC.
//! - Apple's [Key-Value Coding Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/KeyValueCoding/SearchImplementation.html)
//!   explains the algorithm `setValue:forKey:` should follow.
//!
//! See also: [crate::objc], especially the `objects` module.

use super::ns_string::{from_rust_string, to_rust_string};
use super::{NSRange, NSTimeInterval, NSUInteger};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_run_loop::{
    add_perform_request, cancel_all_perform_requests_for_target, cancel_perform_requests,
};
use crate::frameworks::foundation::ns_thread::detach_new_thread_inner;
use crate::libc::semaphore::{host_destroy_semaphore, sem_wait};
use crate::mem::{ConstVoidPtr, MutVoidPtr};
use crate::Environment;
use crate::objc::{
    autorelease, class_getMethodImplementation, id, msg, msg_class, msg_send,
    msg_send_no_type_checking, nil, objc_classes, release, retain, Class, ClassExports, NSZonePtr,
    ObjC, TrivialHostObject, IMP, SEL,
};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSObject

+ (id)alloc {
    msg![env; this allocWithZone:(MutVoidPtr::null())]
}
+ (id)allocWithZone:(NSZonePtr)_zone { // struct _NSZone*
    log_dbg!("[{:?} allocWithZone:]", this);
    env.objc.alloc_object(this, Box::new(TrivialHostObject), &mut env.mem)
}

+ (id)new {
    let new_object: id = msg![env; this alloc];
    msg![env; new_object init]
}

+ (Class)class {
    this
}
+ (bool)isSubclassOfClass:(Class)class {
    env.objc.class_is_subclass_of(this, class)
}

// See the instance method section for the normal versions of these.
+ (id)retain {
    this // classes are not refcounted
}
+ (())release {
    // classes are not refcounted
}
+ (())autorelease {
    // classes are not refcounted
}

+ (bool)instancesRespondToSelector:(SEL)selector {
    env.objc.class_has_method(this, selector)
}

+ (IMP)instanceMethodForSelector:(SEL)selector {
    class_getMethodImplementation(env, this, selector)
}

+ (IMP)methodForSelector:(SEL)selector {
    let metaclass: Class = ObjC::read_isa(this, &env.mem);
    class_getMethodImplementation(env, metaclass, selector)
}

+ (())cancelPreviousPerformRequestsWithTarget:(id)target {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    cancel_all_perform_requests_for_target(env, run_loop, target);
}

+ (())cancelPreviousPerformRequestsWithTarget:(id)target selector:(SEL)selector object:(id)arg {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    cancel_perform_requests(env, run_loop, target, selector, arg);
}

+ (bool)accessInstanceVariablesDirectly {
    true
}

+ (id)description {
    let name = env.objc.get_class_name(this);
    let str = from_rust_string(env, name.to_string());
    autorelease(env, str)
}

+ (id)debugDescription {
    msg![env; this description]
}

+ (id)instanceMethodSignatureForSelector:(SEL)sel {
    // TODO: support `host` method signatures
    if let Some(&sig) = env.objc.class_get_method_signature(this, sel) {
        log_dbg!("instanceMethodSignatureForSelector: '{}' -> {:?}", sel.as_str(&env.mem), env.mem.cstr_at_utf8(sig));
        msg_class![env; NSMethodSignature signatureWithObjCTypes:sig]
    } else {
        nil
    }
}

+ (())initialize {
    // Do nothing
}

+ (bool)conformsToProtocol:(id)_protocol {
    true
}

- (id)init {
    this
}

- (NSUInteger)retainCount {
    env.objc.get_refcount(this).into()
}

- (id)retain {
    log_dbg!("[{:?} retain]", this);
    env.objc.increment_refcount(this);
    this
}
- (())release {
    log_dbg!("[{:?} release]", this);
    if env.objc.decrement_refcount(this) {
        () = msg![env; this dealloc];
    }
}
- (id)autorelease {
    () = msg_class![env; NSAutoreleasePool addObject:this];
    this
}

- (())dealloc {
    log_dbg!("[{:?} dealloc]", this);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (Class)class {
    ObjC::read_isa(this, &env.mem)
}
- (bool)isMemberOfClass:(Class)class {
    let this_class: Class = msg![env; this class];
    class == this_class
}
- (bool)isKindOfClass:(Class)class {
    let this_class: Class = msg![env; this class];
    env.objc.class_is_subclass_of(this_class, class)
}

- (NSUInteger)hash {
    this.to_bits()
}

// To not confuse with isEqualTo:, which is
// a category of NSWhoseSpecifier!
// Reference https://nshipster.com/equality
- (bool)isEqual:(id)other {
    this == other
}

// TODO: Instance description and debugDescription.
// This is not hard to add, but before adding a fallback implementation of it,
// we should make sure all the Foundation classes' overrides of it are there,
// to prevent weird behavior.
// TODO: localized description methods also? (not sure if NSObject has them)

// Helper for NSCopying
- (id)copy {
    msg![env; this copyWithZone:(MutVoidPtr::null())]
}

// Helper for NSMutableCopying
- (id)mutableCopy {
    msg![env; this mutableCopyWithZone:(MutVoidPtr::null())]
}

// NSKeyValueCoding
// https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/KeyValueCoding/SearchImplementation.html
- (())setValue:(id)value
       forKey:(id)key { // NSString*
    let key_string = to_rust_string(env, key);
    let camel_case_key_string = if key_string.is_empty() {
        String::new()
    } else {
        format!("{}{}", key_string.as_bytes()[0].to_ascii_uppercase() as char, &key_string[1..])
    };

    let class: Class = msg![env; this class];

    let invoke_setter = |env: &mut Environment, this: id, sel: SEL, value: id, key: id| -> bool {
        let arg_type_opt = env.objc.class_get_method_signature(class, sel).copied().and_then(|sig_ptr| {
            let sig_str = env.mem.cstr_at_utf8(sig_ptr).unwrap();
            get_arg2_type_from_sig(sig_str).map(|s| s.to_string())
        });
        if let Some(arg_type_str) = arg_type_opt {
            let arg_type = arg_type_str.as_str();
            let is_null = if value == nil {
                true
            } else {
                let ns_null_class = env.objc.get_known_class("NSNull", &mut env.mem);
                let val_class: Class = msg![env; value class];
                env.objc.class_is_subclass_of(val_class, ns_null_class)
            };
            if is_null {
                if arg_type.starts_with('@') || arg_type == "#" || arg_type == ":" || arg_type == "^v" {
                    () = msg_send_no_type_checking(env, (this, sel, nil));
                    return true;
                } else {
                        () = msg![env; this setNilValueForKey:key];
                        return true;
                    }
                }
                match arg_type {
                    "f" => {
                        let num: f32 = msg![env; value floatValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "d" => {
                        let num: f64 = msg![env; value doubleValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "i" | "l" => {
                        let num: i32 = msg![env; value intValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "I" | "L" => {
                        let num: u32 = msg![env; value unsignedIntValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "s" => {
                        let num: i16 = msg![env; value shortValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "S" => {
                        let num: u16 = msg![env; value unsignedShortValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "c" | "B" => {
                        let num: i8 = msg![env; value charValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "C" => {
                        let num: u8 = msg![env; value unsignedCharValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "q" => {
                        let num: i64 = msg![env; value longLongValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    "Q" => {
                        let num: u64 = msg![env; value unsignedLongLongValue];
                        () = msg_send(env, (this, sel, num));
                        return true;
                    }
                    s if s.starts_with("{CGPoint") || s.starts_with("{_CGPoint") => {
                        let pt: CGPoint = msg![env; value CGPointValue];
                        () = msg_send(env, (this, sel, pt));
                        return true;
                    }
                    s if s.starts_with("{CGSize") || s.starts_with("{_CGSize") => {
                        let sz: CGSize = msg![env; value CGSizeValue];
                        () = msg_send(env, (this, sel, sz));
                        return true;
                    }
                    s if s.starts_with("{CGRect") || s.starts_with("{_CGRect") => {
                        let rect: CGRect = msg![env; value CGRectValue];
                        () = msg_send(env, (this, sel, rect));
                        return true;
                    }
                    s if s.starts_with("{_NSRange") || s.starts_with("{NSRange") => {
                        let rng: NSRange = msg![env; value rangeValue];
                        () = msg_send(env, (this, sel, rng));
                        return true;
                    }
                    _ => {
                        () = msg_send_no_type_checking(env, (this, sel, value));
                        return true;
                    }
                }
            }
        () = msg_send_no_type_checking(env, (this, sel, value));
        true
    };

    // Look for the first accessor named set<Key>: or _set<Key>, in that order.
    let sel = env.objc.register_host_selector(format!("set{camel_case_key_string}:"), &mut env.mem);
    if env.objc.class_has_method(class, sel) {
        invoke_setter(env, this, sel, value, key);
        return;
    }

    let under_sel = env.objc.register_host_selector(format!("_set{camel_case_key_string}:"), &mut env.mem);
    if env.objc.class_has_method(class, under_sel) {
        invoke_setter(env, this, under_sel, value, key);
        return;
    }

    // Direct instance variable access if accessInstanceVariablesDirectly is YES
    let sel = env.objc.register_host_selector("accessInstanceVariablesDirectly".to_string(), &mut env.mem);
    let access_direct: bool = msg_send(env, (class, sel));
    if access_direct {
        let ivar_match = env.objc.object_lookup_ivar(&env.mem, this, &format!("_{key_string}"))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("_is{camel_case_key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("{key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("is{camel_case_key_string}")));

        if let Some((ivar_ptr, type_opt)) = ivar_match {
            let is_null = if value == nil {
                true
            } else {
                let ns_null_class = env.objc.get_known_class("NSNull", &mut env.mem);
                let val_class: Class = msg![env; value class];
                env.objc.class_is_subclass_of(val_class, ns_null_class)
            };
            if is_null {
                if let Some(ref t) = type_opt {
                    if t.starts_with('@') || t == "#" || t == ":" || t == "^v" {
                        let old: id = env.mem.read(ivar_ptr.cast());
                        if old != nil {
                            release(env, old);
                        }
                        env.mem.write(ivar_ptr.cast(), nil);
                        return;
                    } else {
                        () = msg![env; this setNilValueForKey:key];
                        return;
                    }
                } else {
                    let old: id = env.mem.read(ivar_ptr.cast());
                    if old != nil {
                        release(env, old);
                    }
                    env.mem.write(ivar_ptr.cast(), nil);
                    return;
                }
            }
            if let Some(ref t) = type_opt {
                match t.as_str() {
                    "f" => {
                        let num: f32 = msg![env; value floatValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "d" => {
                        let num: f64 = msg![env; value doubleValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "i" | "l" => {
                        let num: i32 = msg![env; value intValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "I" | "L" => {
                        let num: u32 = msg![env; value unsignedIntValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "s" => {
                        let num: i16 = msg![env; value shortValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "S" => {
                        let num: u16 = msg![env; value unsignedShortValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "c" | "B" => {
                        let num: i8 = msg![env; value charValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "C" => {
                        let num: u8 = msg![env; value unsignedCharValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "q" => {
                        let num: i64 = msg![env; value longLongValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    "Q" => {
                        let num: u64 = msg![env; value unsignedLongLongValue];
                        env.mem.write(ivar_ptr.cast(), num);
                        return;
                    }
                    s if s.starts_with("{CGPoint") || s.starts_with("{_CGPoint") => {
                        let pt: CGPoint = msg![env; value CGPointValue];
                        env.mem.write(ivar_ptr.cast(), pt);
                        return;
                    }
                    s if s.starts_with("{CGSize") || s.starts_with("{_CGSize") => {
                        let sz: CGSize = msg![env; value CGSizeValue];
                        env.mem.write(ivar_ptr.cast(), sz);
                        return;
                    }
                    s if s.starts_with("{CGRect") || s.starts_with("{_CGRect") => {
                        let rect: CGRect = msg![env; value CGRectValue];
                        env.mem.write(ivar_ptr.cast(), rect);
                        return;
                    }
                    s if s.starts_with("{_NSRange") || s.starts_with("{NSRange") => {
                        let rng: NSRange = msg![env; value rangeValue];
                        env.mem.write(ivar_ptr.cast(), rng);
                        return;
                    }
                    _ => {
                        retain(env, value);
                        let old: id = env.mem.read(ivar_ptr.cast());
                        if old != nil {
                            release(env, old);
                        }
                        env.mem.write(ivar_ptr.cast(), value);
                        return;
                    }
                }
            } else {
                retain(env, value);
                let old: id = env.mem.read(ivar_ptr.cast());
                if old != nil {
                    release(env, old);
                }
                env.mem.write(ivar_ptr.cast(), value);
                return;
            }
        }
    }

    // Fallback to setValue:forUndefinedKey:
    let sel = env.objc.register_host_selector("setValue:forUndefinedKey:".to_string(), &mut env.mem);
    () = msg_send(env, (this, sel, value, key));
}

- (id)valueForKey:(id)key { // NSString*
    let key_string = to_rust_string(env, key);
    let camel_case_key_string = if key_string.is_empty() {
        String::new()
    } else {
        format!("{}{}", key_string.as_bytes()[0].to_ascii_uppercase() as char, &key_string[1..])
    };

    let class: Class = msg![env; this class];

    let candidates: [String; 5] = [
        format!("get{camel_case_key_string}"),
        key_string.to_string(),
        format!("is{camel_case_key_string}"),
        format!("_get{camel_case_key_string}"),
        format!("_{key_string}"),
    ];

    for name in candidates {
        if let Some(sel) = env.objc.lookup_selector(&name) {
            if env.objc.class_has_method(class, sel) {
                if let Some(sig_ptr) = env.objc.class_get_method_signature(class, sel).copied() {
                    let sig_str = env.mem.cstr_at_utf8(sig_ptr).unwrap();
                    let ret_type = get_return_type_from_sig(sig_str);
                    match ret_type {
                        "v" => {
                            () = msg_send_no_type_checking(env, (this, sel));
                            return nil;
                        }
                        "f" => {
                            let val: f32 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithFloat:val];
                        }
                        "d" => {
                            let val: f64 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithDouble:val];
                        }
                        "i" | "l" => {
                            let val: i32 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithInt:val];
                        }
                        "I" | "L" => {
                            let val: u32 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithUnsignedInt:val];
                        }
                        "s" => {
                            let val: i16 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithShort:val];
                        }
                        "S" => {
                            let val: u16 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithUnsignedShort:val];
                        }
                        "c" => {
                            let val: i8 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithChar:val];
                        }
                        "C" => {
                            let val: u8 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithUnsignedChar:val];
                        }
                        "B" => {
                            let val: bool = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithBool:val];
                        }
                        "q" => {
                            let val: i64 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithLongLong:val];
                        }
                        "Q" => {
                            let val: u64 = msg_send(env, (this, sel));
                            return msg_class![env; NSNumber numberWithUnsignedLongLong:val];
                        }
                        s if s.starts_with("{CGPoint") || s.starts_with("{_CGPoint") => {
                            let val: CGPoint = msg_send(env, (this, sel));
                            return msg_class![env; NSValue valueWithCGPoint:val];
                        }
                        s if s.starts_with("{CGSize") || s.starts_with("{_CGSize") => {
                            let val: CGSize = msg_send(env, (this, sel));
                            return msg_class![env; NSValue valueWithCGSize:val];
                        }
                        s if s.starts_with("{CGRect") || s.starts_with("{_CGRect") => {
                            let val: CGRect = msg_send(env, (this, sel));
                            return msg_class![env; NSValue valueWithCGRect:val];
                        }
                        s if s.starts_with("{_NSRange") || s.starts_with("{NSRange") => {
                            let val: NSRange = msg_send(env, (this, sel));
                            return msg_class![env; NSValue valueWithRange:val];
                        }
                        _ => {
                            let val: id = msg_send_no_type_checking(env, (this, sel));
                            return val;
                        }
                    }
                } else {
                    let val: id = msg_send_no_type_checking(env, (this, sel));
                    return val;
                }
            }
        }
    }

    let sel = env.objc.register_host_selector("accessInstanceVariablesDirectly".to_string(), &mut env.mem);
    let access_direct: bool = msg_send(env, (class, sel));
    if access_direct {
        let ivar_match = env.objc.object_lookup_ivar(&env.mem, this, &format!("_{key_string}"))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("_is{camel_case_key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("{key_string}")))
            .or_else(|| env.objc.object_lookup_ivar(&env.mem, this, &format!("is{camel_case_key_string}")));

        if let Some((ivar_ptr, type_opt)) = ivar_match {
            if let Some(ref t) = type_opt {
                match t.as_str() {
                    "f" => {
                        let val: f32 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithFloat:val];
                    }
                    "d" => {
                        let val: f64 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithDouble:val];
                    }
                    "i" | "l" => {
                        let val: i32 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithInt:val];
                    }
                    "I" | "L" => {
                        let num: u32 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithUnsignedInt:num];
                    }
                    "s" => {
                        let val: i16 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithShort:val];
                    }
                    "S" => {
                        let val: u16 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithUnsignedShort:val];
                    }
                    "c" => {
                        let val: i8 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithChar:val];
                    }
                    "C" => {
                        let val: u8 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithUnsignedChar:val];
                    }
                    "B" => {
                        let val: bool = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithBool:val];
                    }
                    "q" => {
                        let val: i64 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithLongLong:val];
                    }
                    "Q" => {
                        let val: u64 = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSNumber numberWithUnsignedLongLong:val];
                    }
                    s if s.starts_with("{CGPoint") || s.starts_with("{_CGPoint") => {
                        let val: CGPoint = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSValue valueWithCGPoint:val];
                    }
                    s if s.starts_with("{CGSize") || s.starts_with("{_CGSize") => {
                        let val: CGSize = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSValue valueWithCGSize:val];
                    }
                    s if s.starts_with("{CGRect") || s.starts_with("{_CGRect") => {
                        let val: CGRect = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSValue valueWithCGRect:val];
                    }
                    s if s.starts_with("{_NSRange") || s.starts_with("{NSRange") => {
                        let val: NSRange = env.mem.read(ivar_ptr.cast());
                        return msg_class![env; NSValue valueWithRange:val];
                    }
                    _ => {
                        let val: id = env.mem.read(ivar_ptr.cast());
                        return val;
                    }
                }
            } else {
                let val: id = env.mem.read(ivar_ptr.cast());
                return val;
            }
        }
    }

    let sel = env.objc.register_host_selector("valueForUndefinedKey:".to_string(), &mut env.mem);
    msg_send(env, (this, sel, key))
}

- (id)valueForUndefinedKey:(id)key {
    let class: Class = if this != nil {
        ObjC::read_isa(this, &env.mem)
    } else {
        nil
    };
    let class_name_string = if class != nil {
        env.objc.try_get_class_name(class).map(|s| s.to_string()).unwrap_or_else(|| format!("{:?}", class))
    } else {
        "nil".to_string()
    };
    let key_string = to_rust_string(env, key);
    log!("Warning: Object {:?} of class {} does not have a getter for key {}", this, class_name_string, key_string);
    nil
}

- (())setValue:(id)_value
forUndefinedKey:(id)key {
    let class: Class = if this != nil {
        ObjC::read_isa(this, &env.mem)
    } else {
        nil
    };
    let class_name_string = if class != nil {
        env.objc.try_get_class_name(class).map(|s| s.to_string()).unwrap_or_else(|| format!("{:?}", class))
    } else {
        "nil".to_string()
    };
    let key_string = to_rust_string(env, key);
    log!("Warning: Object {:?} of class {} does not have a setter for key {}", this, class_name_string, key_string);
}

- (())setNilValueForKey:(id)key {
    let key_string = to_rust_string(env, key);
    log!("Warning: [NSObject setNilValueForKey:{}] called on {:?}", key_string, this);
}

- (())setValuesForKeysWithDictionary:(id)keyedValues {
    if keyedValues == nil {
        return;
    }
    let all_keys: id = msg![env; keyedValues allKeys];
    let count: NSUInteger = msg![env; all_keys count];
    for i in 0..count {
        let key: id = msg![env; all_keys objectAtIndex:i];
        let val: id = msg![env; keyedValues objectForKey:key];
        () = msg![env; this setValue:val forKey:key];
    }
}

- (id)dictionaryWithValuesForKeys:(id)keys {
    let dict: id = msg_class![env; NSMutableDictionary dictionary];
    let count: NSUInteger = msg![env; keys count];
    let ns_null: id = msg_class![env; NSNull null];
    for i in 0..count {
        let key: id = msg![env; keys objectAtIndex:i];
        let val: id = msg![env; this valueForKey:key];
        let val_to_set = if val == nil { ns_null } else { val };
        () = msg![env; dict setObject:val_to_set forKey:key];
    }
    dict
}

- (())willChangeValueForKey:(id)_key { // NSString *
    log_once!("TODO: NSObject willChangeValueForKey:");
}
- (())didChangeValueForKey:(id)_key { // NSString *
    log_once!("TODO: NSObject didChangeValueForKey:");
}

- (())addObserver:(id)_observer forKeyPath:(id)_keyPath options:(NSUInteger)_options context:(ConstVoidPtr)_context {
    log_once!("TODO: [NSObject addObserver:forKeyPath:options:context:]");
}

- (())removeObserver:(id)_observer forKeyPath:(id)_keyPath {
    log_once!("TODO: [NSObject removeObserver:forKeyPath:]");
}

- (())removeObserver:(id)_observer forKeyPath:(id)_keyPath context:(ConstVoidPtr)_context {
    log_once!("TODO: [NSObject removeObserver:forKeyPath:context:]");
}

- (bool)respondsToSelector:(SEL)selector {
    env.objc.object_has_method(&env.mem, this, selector)
}

- (IMP)methodForSelector:(SEL)selector {
    let class: Class = msg![env; this class];
    class_getMethodImplementation(env, class, selector)
}

- (id)methodSignatureForSelector:(SEL)sel {
    let class: Class = msg![env; this class];
    msg![env; class instanceMethodSignatureForSelector:sel]
}

- (id)performSelector:(SEL)sel {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel))
}

- (id)performSelector:(SEL)sel
           withObject:(id)o1 {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel, o1))
}

- (id)performSelector:(SEL)sel
           withObject:(id)o1
           withObject:(id)o2 {
    assert!(!sel.is_null());
    msg_send_no_type_checking(env, (this, sel, o1, o2))
}

- (())performSelectorInBackground:(SEL)sel
                       withObject:(id)arg {
    detach_new_thread_inner(env, sel, this, arg, /* tolerate_type_mismatch: */ true)
}

- (())performSelector:(SEL)sel withObject:(id)arg afterDelay:(NSTimeInterval)delay {
    let run_loop: id = msg_class![env; NSRunLoop currentRunLoop];
    add_perform_request(env, run_loop, this, sel, arg, Some(delay), false);
}

- (())performSelectorOnMainThread:(SEL)sel withObject:(id)arg waitUntilDone:(bool)wait {
    log_dbg!("performSelectorOnMainThread:{} withObject:{:?} waitUntilDone:{}", sel.as_str(&env.mem), arg, wait);
    if wait && env.current_thread == 0 {
        if sel.as_str(&env.mem).ends_with(':') {
            () = msg_send(env, (this, sel, arg));
        } else {
            assert!(arg.is_null());
            () = msg_send(env, (this, sel));
        }
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.POP") && (sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie").unwrap()) && wait {
        log!("Applying game-specific hack for PoP: WW: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.Asphalt5") && (sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie:").unwrap()) && wait {
        log!("Applying game-specific hack for Asphalt5: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.SplinterCell") && sel == env.objc.lookup_selector("startMovie:").unwrap() && wait {
        log!("Applying game-specific hack for SplinterCell: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.AssassinsCreed") && sel == env.objc.lookup_selector("moviePlayerInit:").unwrap() && wait {
        log!("Applying game-specific hack for AssassinsCreed: ignoring performSelectorOnMainThread:SEL(moviePlayerInit:) waitUntilDone:true");
        return;
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.Ferrari") && wait {
        if sel == env.objc.lookup_selector("startMovie:").unwrap() {
            log!("Applying game-specific hack for Ferrari GT: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
            return;
        }
        if sel == env.objc.lookup_selector("initTextInput:").unwrap() || sel == env.objc.lookup_selector("removeTextField:").unwrap() {
            log!("Applying game-specific hack for Ferrari GT: performing performSelectorOnMainThread:SEL({}) waitUntilDone:true on thread {}", sel.as_str(&env.mem), env.current_thread);
            () = msg_send(env, (this, sel, arg));
            return;
        }
    }
    if env.bundle.bundle_identifier().starts_with("com.gameloft.HOS2") && wait {
        if sel == env.objc.lookup_selector("loadMovie:").unwrap() || sel == env.objc.lookup_selector("sendGameInfo").unwrap() || sel == env.objc.lookup_selector("setStatusBar:").unwrap() {
            log!("Applying game-specific hack for HOS2: performing performSelectorOnMainThread:SEL({}) waitUntilDone:true on thread {}", sel.as_str(&env.mem), env.current_thread);
            if sel.as_str(&env.mem).ends_with(':') {
                () = msg_send(env, (this, sel, arg));
            } else {
                assert!(arg.is_null());
                () = msg_send(env, (this, sel));
            }
            return;
        }
        if sel == env.objc.lookup_selector("startMovie:").unwrap() || sel == env.objc.lookup_selector("stopMovie:").unwrap() {
            log!("Applying game-specific hack for HOS2: ignoring performSelectorOnMainThread:SEL({}) waitUntilDone:true", sel.as_str(&env.mem));
            return;
        }
    }

    let run_loop: id = msg_class![env; NSRunLoop mainRunLoop];
    let sem = add_perform_request(env, run_loop, this, sel, arg, None, wait);
    if wait {
        sem_wait(env, sem);
        host_destroy_semaphore(env, sem);
    }
}

// UINibLoadingAdditions protocol
- (())awakeFromNib {
    // no-op
}

- (bool)conformsToProtocol:(id)_protocol {
    true
}

@end

};

fn get_return_type_from_sig(sig: &str) -> &str {
    if sig.starts_with('{') {
        let mut depth = 0;
        for (i, c) in sig.char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return &sig[..=i];
                }
            }
        }
        sig
    } else if sig.starts_with("@\"") {
        if let Some(end_quote) = sig[2..].find('"') {
            &sig[..2 + end_quote + 1]
        } else {
            "@"
        }
    } else if sig.starts_with('@') {
        "@"
    } else {
        let end = sig.find(|c: char| c.is_ascii_digit() || c == '@').unwrap_or(sig.len());
        &sig[..end]
    }
}

fn get_arg2_type_from_sig(sig: &str) -> Option<&str> {
    let colon = sig.find(':')?;
    let rest = &sig[colon + 1..];
    let after_offset = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    if after_offset.is_empty() {
        return None;
    }
    if after_offset.starts_with('{') {
        let mut depth = 0;
        for (i, c) in after_offset.char_indices() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(&after_offset[..=i]);
                }
            }
        }
        Some(after_offset)
    } else if after_offset.starts_with("@\"") {
        if let Some(end_quote) = after_offset[2..].find('"') {
            Some(&after_offset[..2 + end_quote + 1])
        } else {
            Some("@")
        }
    } else if after_offset.starts_with('@') {
        Some("@")
    } else {
        let end = after_offset.find(|c: char| c.is_ascii_digit() || c == '@').unwrap_or(after_offset.len());
        Some(&after_offset[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_parsing() {
        assert_eq!(get_return_type_from_sig("v8@0:4"), "v");
        assert_eq!(get_return_type_from_sig("f8@0:4"), "f");
        assert_eq!(get_return_type_from_sig("d8@0:4"), "d");
        assert_eq!(get_return_type_from_sig("i8@0:4"), "i");
        assert_eq!(get_return_type_from_sig("@8@0:4"), "@");
        assert_eq!(get_return_type_from_sig("@\"NSString\"8@0:4"), "@\"NSString\"");
        assert_eq!(
            get_return_type_from_sig("{CGRect={CGPoint=ff}{CGSize=ff}}8@0:4"),
            "{CGRect={CGPoint=ff}{CGSize=ff}}"
        );

        assert_eq!(get_arg2_type_from_sig("v8@0:4"), None);
        assert_eq!(get_arg2_type_from_sig("v12@0:4f8"), Some("f"));
        assert_eq!(get_arg2_type_from_sig("v12@0:4d8"), Some("d"));
        assert_eq!(get_arg2_type_from_sig("v12@0:4i8"), Some("i"));
        assert_eq!(get_arg2_type_from_sig("v12@0:4@8"), Some("@"));
        assert_eq!(get_arg2_type_from_sig("v12@0:4@\"NSString\"8"), Some("@\"NSString\""));
        assert_eq!(
            get_arg2_type_from_sig("v24@0:4{CGPoint=ff}8"),
            Some("{CGPoint=ff}")
        );
        // Also test signatures without explicit stack offset numbers
        assert_eq!(get_arg2_type_from_sig("v@:f"), Some("f"));
        assert_eq!(get_arg2_type_from_sig("v@:i"), Some("i"));
        assert_eq!(get_arg2_type_from_sig("v@:@"), Some("@"));
    }
}

