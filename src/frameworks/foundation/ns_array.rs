/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSArray` class cluster, including `NSMutableArray`.

use super::ns_enumerator::{fast_enumeration_helper, NSFastEnumerationState};
use super::ns_property_list_serialization::{
    deserialize_plist_from_file, NSPropertyListBinaryFormat_v1_0,
};
use super::{
    _nib_archive_decoder, ns_keyed_unarchiver, ns_string, ns_url, NSComparisonResult, NSNotFound,
    NSRange, NSUInteger,
};
use crate::abi::{CallFromHost, DotDotDot, GuestFunction};
use crate::frameworks::foundation::ns_keyed_archiver::{
    encode_object, get_value_to_encode_for_current_key,
};
use crate::fs::GuestPath;
use crate::libc::stdlib::qsort::qsort_generic;
use crate::mem::{ConstPtr, MutPtr, MutVoidPtr, Ptr};
use crate::objc::{
    autorelease, id, msg, msg_class, msg_send, nil, objc_classes, release, retain, Class,
    ClassExports, HostObject, NSZonePtr, SEL,
};
use crate::Environment;

struct ObjectEnumeratorHostObject {
    /// the enumerated collection, NSArray *
    array: id,
    /// an iterator
    iterator: std::vec::IntoIter<id>,
}
impl HostObject for ObjectEnumeratorHostObject {}

/// Belongs to _touchHLE_NSArray
#[derive(Debug, Default)]
pub(super) struct ArrayHostObject {
    pub(super) array: Vec<id>,
}
impl HostObject for ArrayHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSArray is an abstract class. A subclass must provide:
// - (NSUInteger)count;
// - (id)objectAtIndex:(NSUInteger)index;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _touchHLE_NSArray.
@implementation NSArray: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSArray might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSArray", &mut env.mem));
    msg_class![env; _touchHLE_NSArray allocWithZone:zone]
}

+ (id)array {
    let array: id = msg![env; this new];
    autorelease(env, array)
}

+ (id)arrayWithArray:(id)other { // NSArray*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithArray:other];
    autorelease(env, array)
}

// These probably comes from some category related to plists.
+ (id)arrayWithContentsOfFile:(id)path { // NSString*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithContentsOfFile:path];
    autorelease(env, array)
}
+ (id)arrayWithContentsOfURL:(id)url { // NSURL*
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithContentsOfURL:url];
    autorelease(env, array)
}

+ (id)arrayWithObject:(id)object {
    retain(env, object);
    let objects = vec![object];
    let array = from_vec(env, objects);
    autorelease(env, array)
}
+ (id)arrayWithObjects:(id)first_obj, ...args {
    let objects = if first_obj == nil {
        vec![]
    } else {
        retain(env, first_obj);
        let mut objects = vec![first_obj];
        let mut varargs = args.start();
        loop {
            let next_arg: id = varargs.next(env);
            if next_arg.is_null() {
                break;
            }
            retain(env, next_arg);
            objects.push(next_arg);
        }
        objects
    };
    let array = from_vec(env, objects);
    autorelease(env, array)
}
+ (id)arrayWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    let array: id = msg![env; this alloc];
    let array: id = msg![env; array initWithObjects:objects_ptr count:count];
    autorelease(env, array)
}

// These probably comes from some category related to plists.
- (id)initWithContentsOfFile:(id)path { // NSString*
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ true,
    )
}
- (id)initWithContentsOfURL:(id)url { // NSURL*
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    deserialize_plist_from_file(env, &path, /* array_expected: */ true)
}

- (bool)writeToFile:(id)path // NSString*
         atomically:(bool)atomically {
    let error_desc: MutPtr<id> = Ptr::null();
    let data: id = msg_class![env; NSPropertyListSerialization
            dataFromPropertyList:this
                          format:NSPropertyListBinaryFormat_v1_0
                errorDescription:error_desc];
    let res = msg![env; data writeToFile:path atomically:atomically];
    log_dbg!(
        "[(NSArray *){:?} writeToFile:{:?} atomically:{}] -> {}",
        this,
        ns_string::to_rust_string(env, path),
        atomically,
        res
    );
    res
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (NSUInteger)indexOfObject:(id)object {
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let equal: bool = msg![env; object isEqual:curr_object];
        if equal {
            return i;
        }
    }
    NSNotFound as NSUInteger
}

- (NSUInteger)indexOfObjectIdenticalTo:(id)object {
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        if object == curr_object {
            return i;
        }
    }
    NSNotFound as NSUInteger
}

- (NSUInteger)indexOfObjectIdenticalTo:(id)object inRange:(NSRange)range {
    for i in range.location..(range.location + range.length) {
        let curr_object: id = msg![env; this objectAtIndex:i];
        if object == curr_object {
            return i;
        }
    }
    NSNotFound as NSUInteger
}

- (bool)containsObject:(id)object {
    let idx: NSUInteger = msg![env; this indexOfObject:object];
    idx != NSNotFound as NSUInteger
}

- (id)firstObject {
    let size: NSUInteger = msg![env; this count];
    if size == 0 {
        return nil;
    }
    msg![env; this objectAtIndex:0u32]
}

- (id)lastObject {
    let size: NSUInteger = msg![env; this count];
    if size == 0 {
        return nil;
    }
    msg![env; this objectAtIndex:(size - 1)]
}

- (id)componentsJoinedByString:(id)str { // NSString *
    let res: id = msg_class![env; NSMutableString new];
    let count: NSUInteger = msg![env; this count];
    if count == 0 {
        autorelease(env, res);
        return res;
    }
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let curr_desc: id = msg![env; curr_object description];
        () = msg![env; res appendString:curr_desc];
        if i != count-1 {
            () = msg![env; res appendString:str];
        }
    }
    let res_imm = msg![env; res copy];
    release(env, res);
    autorelease(env, res_imm)
}

- (id)sortedArrayUsingFunction:(GuestFunction)comparator
                       context:(MutVoidPtr)context {
    let array = msg![env; this mutableCopy];
    () = msg![env; array sortUsingFunction:comparator context:context];
    let array_imm = msg![env; array copy];
    release(env, array);
    autorelease(env, array_imm)
}

- (NSUInteger)hash {
    // TODO: define better hash
    msg![env; this count]
}
- (bool)isEqual:(id)other {
    if this == other {
        return true;
    }
    let class: Class = msg_class![env; NSArray class];
    if !msg![env; other isKindOfClass:class] {
        return false;
    }
    msg![env; this isEqualToArray:other]
}
- (bool)isEqualToArray:(id)other { // NSArray *
    if other == nil {
        return false;
    }
    let count: NSUInteger = msg![env; this count];
    let other_count: NSUInteger = msg![env; other count];
    if count != other_count {
        return false;
    }
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let curr_other_object: id = msg![env; other objectAtIndex:i];
        let equal: bool = msg![env; curr_object isEqual:curr_other_object];
        if !equal {
            return false;
        }
    }
    true
}

- (())enumerateObjectsUsingBlock:(id)block {
    if block == nil {
        return;
    }
    let count: NSUInteger = msg![env; this count];
    if count == 0 {
        return;
    }
    let stop_ptr: MutPtr<u8> = env.mem.alloc_and_write(0u8);
    let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
    let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);

    for i in 0..count {
        let stop: u8 = env.mem.read(stop_ptr);
        if stop != 0 {
            break;
        }
        let obj: id = msg![env; this objectAtIndex:i];
        let (): () = invoke.call_from_host(env, (block, obj, i, stop_ptr));
    }

    env.mem.free(stop_ptr.cast());
}

- (())enumerateObjectsWithOptions:(NSUInteger)_opts
                        usingBlock:(id)block {
    () = msg![env; this enumerateObjectsUsingBlock:block];
}

- (id)initWithObjects:(ConstPtr<id>)_objects_ptr count:(NSUInteger)_count {
    todo!("TODO: Implement [NSArray initWithObjects:count:] for custom subclasses")
}

- (id)arrayByAddingObjectsFromArray:(id)other_array { // NSArray*
    let copy: id = msg![env; this mutableCopy];
    if other_array != nil {
        () = msg![env; copy addObjectsFromArray:other_array];
    }
    let res: id = msg![env; copy copy];
    release(env, copy);
    autorelease(env, res)
}

- (id)arrayByAddingObject:(id)an_object {
    assert_ne!(an_object, nil, "arrayByAddingObject: object cannot be nil");
    let copy: id = msg![env; this mutableCopy];
    () = msg![env; copy addObject:an_object];
    let res: id = msg![env; copy copy];
    release(env, copy);
    autorelease(env, res)
}

- (id)sortedArrayUsingSelector:(SEL)comparator {
    let array: id = msg![env; this mutableCopy];
    () = msg![env; array sortUsingSelector:comparator];
    let array_imm: id = msg![env; array copy];
    release(env, array);
    autorelease(env, array_imm)
}

- (id)subarrayWithRange:(NSRange)range {
    let count: NSUInteger = msg![env; this count];
    let start = range.location;
    let end = range.location.checked_add(range.length).expect("range overflow");
    assert!(end <= count, "subarrayWithRange: range {:?} out of bounds for count {}", range, count);
    let mut tmp = Vec::with_capacity(range.length as usize);
    for i in start..end {
        let obj: id = msg![env; this objectAtIndex:i];
        retain(env, obj);
        tmp.push(obj);
    }
    let res = from_vec(env, tmp);
    autorelease(env, res)
}

- (())makeObjectsPerformSelector:(SEL)sel {
    let count: NSUInteger = msg![env; this count];
    let mut objects = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let obj: id = msg![env; this objectAtIndex:idx];
        if obj != nil {
            retain(env, obj);
            objects.push(obj);
        }
    }
    for obj in objects {
        let _: id = msg![env; obj performSelector:sel];
        release(env, obj);
    }
}

- (())makeObjectsPerformSelector:(SEL)sel withObject:(id)argument {
    if argument != nil {
        retain(env, argument);
    }
    let count: NSUInteger = msg![env; this count];
    let mut objects = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let obj: id = msg![env; this objectAtIndex:idx];
        if obj != nil {
            retain(env, obj);
            objects.push(obj);
        }
    }
    for obj in objects {
        let _: id = msg![env; obj performSelector:sel withObject:argument];
        release(env, obj);
    }
    if argument != nil {
        release(env, argument);
    }
}

@end

// NSMutableArray is an abstract class. A subclass must provide everything
// NSArray provides, plus:
// - (void)insertObject:(id)object atIndex:(NSUInteger)index;
// - (void)removeObjectAtIndex:(NSUInteger)index;
// - (void)addObject:(id)object;
// - (void)removeLastObject
// - (void)replaceObjectAtIndex:(NSUInteger)index withObject:(id)object;
// Note that it inherits from NSArray, so we must ensure we override any default
// methods that would be inappropriate for mutability.
@implementation NSMutableArray: NSArray

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSArray might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableArray", &mut env.mem));
    msg_class![env; _touchHLE_NSMutableArray allocWithZone:zone]
}

+ (id)arrayWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

+ (id)arrayWithArray:(id)array {
    let new: id = msg![env; this alloc];
    () = msg![env; new addObjectsFromArray:array];
    autorelease(env, new)
}

+ (id)arrayWithObjects:(id)firstObj, ...args {
    retain(env, firstObj);
    let mut objects = vec![firstObj];
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg.is_null() {
            break;
        }
        retain(env, next_arg);
        objects.push(next_arg);
    }
    let array = mutable_from_vec(env, objects);
    autorelease(env, array)
}

// These probably comes from some category related to plists.
- (id)initWithContentsOfFile:(id)path { // NSString*
    release(env, this);
    let path = ns_string::to_rust_string(env, path);
    let tmp = deserialize_plist_from_file(
        env,
        GuestPath::new(&path),
        /* array_expected: */ true,
    );
    if tmp == nil {
        return nil;
    }
    // We should respect mutability of the top most container!
    let res = msg_class![env; NSMutableArray alloc];
    let res = msg![env; res initWithArray:tmp];
    release(env, tmp);
    res
}
- (id)initWithContentsOfURL:(id)url { // NSURL*
    release(env, this);
    let path = ns_url::to_rust_path(env, url);
    let tmp = deserialize_plist_from_file(env, &path, /* array_expected: */ true);
    if tmp == nil {
        return nil;
    }
    // We should respect mutability of the top most container!
    let res = msg_class![env; NSMutableArray alloc];
    let res = msg![env; res initWithArray:tmp];
    release(env, tmp);
    res
}

- (())addObjectsFromArray:(id)other { // NSArray*
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let other: id = msg_class![env; NSArray alloc];
    let other: id = msg![env; other initWithArray:this];
    other
}

- (id)initWithObjects:(ConstPtr<id>)_objects_ptr count:(NSUInteger)_count {
    todo!("TODO: Implement [NSMutableArray initWithObjects:count:] for custom subclasses")
}

- (())removeObjectsInRange:(NSRange)_range {
    todo!("TODO: Implement [NSMutableArray removeObjectsInRange:] for custom subclasses")
}

- (())replaceObjectsInRange:(NSRange)range withObjectsFromArray:(id)other_array {
    for _ in 0..range.length {
        () = msg![env; this removeObjectAtIndex:(range.location)];
    }
    if other_array != nil {
        let count: NSUInteger = msg![env; other_array count];
        for i in 0..count {
            let obj: id = msg![env; other_array objectAtIndex:i];
            () = msg![env; this insertObject:obj atIndex:(range.location + i)];
        }
    }
}

@end

// Our private subclass that is the single implementation of NSArray for the
// time being.
@implementation _touchHLE_NSArray: NSArray

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(ArrayHostObject {
        array: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

- (id)initWithArray:(id)array { // NSArray*
    msg![env; this initWithArray:array copyItems:false]
}

- (id)initWithArray:(id)array copyItems:(bool)copy_items {
    let mut objects = Vec::new();
    let enumerator: id = msg![env; array objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        let obj = if copy_items {
            let copied: id = msg![env; next copy];
            copied
        } else {
            retain(env, next);
            next
        };
        objects.push(obj);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    init_with_objects_inner(env, this, first_obj, args);
    this
}

- (id)initWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    let mut objects = Vec::new();
    for i in 0..count {
        let obj: id = env.mem.read(objects_ptr + i);
        retain(env, obj);
        objects.push(obj);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

- (())dealloc {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);

    for object in array {
        release(env, object);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    mutable_copy_inner(env, this)
}

- (id)objectEnumerator { // NSEnumerator*
    object_enumerator_inner(env, this)
}
- (id)reverseObjectEnumerator { // NSEnumerator*
    reverse_object_enumerator_inner(env, this)
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    let count: NSUInteger = msg![env; this count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; this objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

// TODO: more init methods, etc

- (NSUInteger)count {
    env.objc.borrow::<ArrayHostObject>(this).array.len().try_into().unwrap()
}
- (id)objectAtIndex:(NSUInteger)index {
    // TODO: throw real exception rather than panic if out-of-bounds?
    env.objc.borrow::<ArrayHostObject>(this).array[index as usize]
}

- (id)description {
    build_description(env, this)
}

@end

// Special variant for use by CFArray with NULL callbacks: objects aren't
// necessarily Objective-C objects and won't be retained/released.
@implementation _touchHLE_NSArray_non_retaining: _touchHLE_NSArray

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

@implementation _touchHLE_NSArray_ObjectEnumerator: NSEnumerator

- (id)nextObject {
    let host_obj = env.objc.borrow_mut::<ObjectEnumeratorHostObject>(this);
    host_obj.iterator.next().unwrap_or(nil)
}

- (())dealloc {
    let host_obj = env.objc.borrow::<ObjectEnumeratorHostObject>(this);
    release(env, host_obj.array);
    env.objc.dealloc_object(this, &mut env.mem)
}

@end

// Our private subclass that is the single implementation of NSMutableArray for
// the time being.
@implementation _touchHLE_NSMutableArray: NSMutableArray

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(ArrayHostObject {
        array: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCapacity:(NSUInteger)capacity {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.reserve(capacity as usize);
    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    init_with_objects_inner(env, this, first_obj, args);
    this
}

- (id)initWithObjects:(ConstPtr<id>)objects_ptr count:(NSUInteger)count {
    let mut objects = Vec::new();
    for i in 0..count {
        let obj: id = env.mem.read(objects_ptr + i);
        retain(env, obj);
        objects.push(obj);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

- (id)initWithArray:(id)array { // NSArray*
    msg![env; this initWithArray:array copyItems:false]
}

- (id)initWithArray:(id)array copyItems:(bool)copy_items {
    let mut objects = Vec::new();
    let enumerator: id = msg![env; array objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        let obj = if copy_items {
            let copied: id = msg![env; next copy];
            copied
        } else {
            retain(env, next);
            next
        };
        objects.push(obj);
    }
    env.objc.borrow_mut::<ArrayHostObject>(this).array = objects;
    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    let arr: id = msg_class![env; NSArray alloc];
    let array = env.objc.borrow::<ArrayHostObject>(this).array.clone();
    for &object in &array {
        retain(env, object);
    }
    env.objc.borrow_mut::<ArrayHostObject>(arr).array = array;
    arr
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)_zone {
    mutable_copy_inner(env, this)
}

- (())dealloc {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);

    for object in array {
        release(env, object);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)objectEnumerator { // NSEnumerator*
    object_enumerator_inner(env, this)
}
- (id)reverseObjectEnumerator { // NSEnumerator*
    reverse_object_enumerator_inner(env, this)
}

- (())sortUsingFunction:(GuestFunction)comparator
                context:(MutVoidPtr)context {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let mut array = std::mem::take(&mut host_object.array);
    let len = array.len().try_into().unwrap();
    let mut user_data = (env, &mut array);
    qsort_generic(
        &mut user_data,
        len,
        &mut |(env, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            comparator.call_from_host(env, (array[l], array[r], context))
        },
        &mut |(_, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            array.swap(l, r);
        },
    );
    let (env, _) = user_data;
    env.objc.borrow_mut::<ArrayHostObject>(this).array = array;
}

- (())sortUsingSelector:(SEL)comparator {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let mut array = std::mem::take(&mut host_object.array);
    let len = array.len().try_into().unwrap();
    let mut user_data = (env, &mut array);
    qsort_generic(
        &mut user_data,
        len,
        &mut |(env, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            let res: NSComparisonResult = msg_send(env, (array[l], comparator, array[r]));
            res
        },
        &mut |(_, array), l, r| {
            let (l, r): (usize, usize) = (l.try_into().unwrap(), r.try_into().unwrap());
            array.swap(l, r);
        },
    );

    let (env, _) = user_data;
    env.objc.borrow_mut::<ArrayHostObject>(this).array = array;
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // TODO: check that array wasn't mutated!
    let count: NSUInteger = msg![env; this count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; this objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

- (NSUInteger)count {
    env.objc.borrow::<ArrayHostObject>(this).array.len().try_into().unwrap()
}
- (id)objectAtIndex:(NSUInteger)index {
    // TODO: throw real exception rather than panic if out-of-bounds?
    env.objc.borrow::<ArrayHostObject>(this).array[index as usize]
}

- (id)description {
    build_description(env, this)
}

// TODO: more mutation methods

- (())insertObject:(id)object
           atIndex:(NSUInteger)index {
    retain(env, object);
    env.objc.borrow_mut::<ArrayHostObject>(this).array.insert(index as usize, object);
}

- (())addObject:(id)object {
    retain(env, object);
    env.objc.borrow_mut::<ArrayHostObject>(this).array.push(object);
}

- (())removeObject:(id)object {
    let mut to_remove = Vec::new();
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        let equal: bool = msg![env; object isEqual:curr_object];
        if equal {
            to_remove.push(i);
        }
    }
    // TODO: runtime here is O(n^2), it could be O(n) instead
    for i in to_remove {
        () = msg![env; this removeObjectAtIndex:i];
    }
}

- (())removeObjectIdenticalTo:(id)object {
    let mut to_remove = Vec::new();
    let count: NSUInteger = msg![env; this count];
    for i in 0..count {
        let curr_object: id = msg![env; this objectAtIndex:i];
        if object == curr_object {
            to_remove.push(i);
        }
    }
    for i in to_remove.into_iter().rev() {
        () = msg![env; this removeObjectAtIndex:i];
    }
}

- (())removeObjectsInRange:(NSRange)range {
    let host_obj = env.objc.borrow_mut::<ArrayHostObject>(this);
    let start = range.location as usize;
    let end = (range.location + range.length) as usize;
    assert!(end <= host_obj.array.len());
    let drained: Vec<id> = host_obj.array.drain(start..end).collect();
    for obj in drained {
        release(env, obj);
    }
}

- (())removeObjectAtIndex:(NSUInteger)index {
    let object = env.objc.borrow_mut::<ArrayHostObject>(this).array.remove(index as usize);
    release(env, object)
}

- (())replaceObjectAtIndex:(NSUInteger)index withObject:(id)obj {
    retain(env, obj);
    let object = std::mem::replace(&mut env.objc.borrow_mut::<ArrayHostObject>(this).array[index as usize], obj);
    release(env, object);
}

- (())removeLastObject {
    let object = env.objc.borrow_mut::<ArrayHostObject>(this).array.pop().unwrap();
    release(env, object)
}

- (())removeAllObjects {
    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);
    for object in array {
        release(env, object);
    }

    env.objc.borrow_mut::<ArrayHostObject>(this).array = Vec::new()
}

- (())removeObjectsInArray:(id)other { // NSArray*
    let other_count: NSUInteger = msg![env; other count];
    if other_count == 0 {
        return;
    }

    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(this);
    let array = std::mem::take(&mut host_object.array);

    // Since removing objects individually has to iterate the whole array
    // for each object (with [array removeObject:obj]), so this just saves some
    // runtime costs.
    let other_array_set: id = msg_class![env; NSSet alloc];
    let other_array_set: id = msg![env; other_array_set initWithArray:other];
    let mut new_array = Vec::with_capacity(array.len());
    for object in array {
        let contains: bool = msg![env; other_array_set containsObject:object];
        if contains {
            release(env, object);
        } else {
            new_array.push(object);
        }
    }

    env.objc.borrow_mut::<ArrayHostObject>(this).array = new_array;
    release(env, other_array_set);
}

@end

// Special variant for use by CFArray with NULL callbacks: objects aren't
// necessarily Objective-C objects and won't be retained/released.
@implementation _touchHLE_NSMutableArray_non_retaining: _touchHLE_NSMutableArray

- (())dealloc {
    env.objc.dealloc_object(this, &mut env.mem)
}

- (())addObject:(id)object {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.push(object);
}

- (())removeObjectAtIndex:(NSUInteger)index {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.remove(index as usize);
}

- (())removeLastObject {
    env.objc.borrow_mut::<ArrayHostObject>(this).array.pop().unwrap();
}

@end

};

/// Shortcut for host code, roughly equivalent to
/// `[[NSArray alloc] initWithObjects:count]` but without copying.
/// The elements should already be "retained by" the `Vec`.
pub fn from_vec(env: &mut Environment, objects: Vec<id>) -> id {
    let array: id = msg_class![env; NSArray alloc];
    env.objc.borrow_mut::<ArrayHostObject>(array).array = objects;
    array
}

/// Shortcut for host code, roughly equivalent to
/// `[[NSMutableArray alloc] initWithObjects:count]` but without copying.
/// The elements should already be "retained by" the `Vec`.
pub fn mutable_from_vec(env: &mut Environment, objects: Vec<id>) -> id {
    let array: id = msg_class![env; NSMutableArray alloc];
    env.objc.borrow_mut::<ArrayHostObject>(array).array = objects;
    array
}

/// A helper to build a description NSString
/// for a NSArray or a NSMutableArray.
fn build_description(env: &mut Environment, arr: id) -> id {
    // According to docs, this description should be formatted as property list.
    // But by the same docs, it's meant to be used for debugging purposes only.
    let desc: id = msg_class![env; NSMutableString new];
    let prefix: id = ns_string::from_rust_string(env, "(\n".to_string());
    () = msg![env; desc appendString:prefix];
    release(env, prefix);
    let values: Vec<id> = env.objc.borrow_mut::<ArrayHostObject>(arr).array.clone();
    for value in values {
        let value_desc: id = msg![env; value description];
        // TODO: respect nesting and padding
        let format = format!("\t{},\n", ns_string::to_rust_string(env, value_desc));
        let format = ns_string::from_rust_string(env, format);
        () = msg![env; desc appendString:format];
        release(env, format);
    }
    let suffix: id = ns_string::from_rust_string(env, ")".to_string());
    () = msg![env; desc appendString:suffix];
    release(env, suffix);
    let desc_imm = msg![env; desc copy];
    release(env, desc);
    autorelease(env, desc_imm)
}

/// A shared objectEnumerator helper method.
fn object_enumerator_inner(env: &mut Environment, arr: id) -> id {
    let array_host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    let vec = array_host_object.array.to_vec();
    object_enumerator_inner_helper(env, arr, vec)
}

/// A shared reverseObjectEnumerator helper method.
fn reverse_object_enumerator_inner(env: &mut Environment, arr: id) -> id {
    let array_host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    // TODO: avoid copying?
    let vec = array_host_object
        .array
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    object_enumerator_inner_helper(env, arr, vec)
}

fn object_enumerator_inner_helper(env: &mut Environment, arr: id, vec: Vec<id>) -> id {
    let host_object = Box::new(ObjectEnumeratorHostObject {
        array: arr,
        iterator: vec.into_iter(),
    });
    retain(env, arr);
    let class = env
        .objc
        .get_known_class("_touchHLE_NSArray_ObjectEnumerator", &mut env.mem);
    let enumerator = env.objc.alloc_object(class, host_object, &mut env.mem);
    autorelease(env, enumerator)
}

fn mutable_copy_inner(env: &mut Environment, arr: id) -> id {
    let mut_arr: id = msg_class![env; NSMutableArray alloc];
    let array = env.objc.borrow::<ArrayHostObject>(arr).array.clone();
    for &object in &array {
        retain(env, object);
    }
    env.objc.borrow_mut::<ArrayHostObject>(mut_arr).array = array;
    mut_arr
}

fn init_with_coder_inner(env: &mut Environment, arr: id, coder: id) -> id {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _touchHLE_NIBArchiveDecoder class];
    // It seems that every NSArray item in an NSKeyedArchiver plist looks like:
    // {
    //   "$class" => (uid of NSArray class goes here),
    //   "NS.objects" => [
    //     // objects here
    //   ]
    // }
    // Presumably we need to call a `decodeFooBarForKey:` method on the NSCoder
    // here, passing in an NSString for "NS.objects". There is no method for
    // arrays though (maybe it's `decodeObjectForKey:`), and in any case
    // allocating an NSString here would be inconvenient, so let's just take a
    // shortcut.
    let objects = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_array(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_array(env, coder)
    } else {
        unimplemented!()
    };

    let host_object: &mut ArrayHostObject = env.objc.borrow_mut(arr);
    assert!(host_object.array.is_empty());
    host_object.array = objects; // objects are already retained
    arr
}

fn encode_with_coder_inner(env: &mut Environment, arr: id, coder: id) {
    let host_obj: ArrayHostObject = std::mem::take(env.objc.borrow_mut(arr));
    let mut encoded_vals = vec![];
    for v in &host_obj.array {
        // TODO: support other type of coders, not only NSKeyedArchiver
        let vv = encode_object(env, coder, *v);
        encoded_vals.push(plist::Value::Uid(vv));
    }
    *env.objc.borrow_mut(arr) = host_obj;

    let scope = get_value_to_encode_for_current_key(env, coder);
    scope.insert("NS.objects".to_string(), plist::Value::Array(encoded_vals));
}

/// Helper for initWithObjects.
fn init_with_objects_inner(env: &mut Environment, arr: id, first_obj: id, args: DotDotDot) {
    retain(env, first_obj);
    let mut objects = vec![first_obj];
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg.is_null() {
            break;
        }
        retain(env, next_arg);
        objects.push(next_arg);
    }
    env.objc.borrow_mut::<ArrayHostObject>(arr).array = objects;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
    use crate::objc::{id, msg, msg_class, nil, SEL};
    use crate::Environment;

    static TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn get_test_environment() -> (std::sync::MutexGuard<'static, ()>, Environment) {
        let guard = TEST_ENV_MUTEX.lock().unwrap();
        let env = loop {
            match Environment::new_for_test() {
                Ok(env) => break env,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        };
        (guard, env)
    }

    #[test]
    fn test_array_make_objects_perform_selector_with_uiview_subviews() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        // Matches MapleStory: -[ItemCell setItem:] at 0x91308:
        // [self->enchantHistoryMarks makeObjectsPerformSelector:@selector(removeFromSuperview)];
        let parent: id = msg_class![env; UIView alloc];
        let parent: id = msg![env; parent initWithFrame:(CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 100.0, height: 100.0 } })];

        let child1: id = msg_class![env; UIView alloc];
        let child1: id = msg![env; child1 initWithFrame:(CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: 10.0, height: 10.0 } })];

        let child2: id = msg_class![env; UIView alloc];
        let child2: id = msg![env; child2 initWithFrame:(CGRect { origin: CGPoint { x: 10.0, y: 0.0 }, size: CGSize { width: 10.0, height: 10.0 } })];

        let child3: id = msg_class![env; UIView alloc];
        let child3: id = msg![env; child3 initWithFrame:(CGRect { origin: CGPoint { x: 20.0, y: 0.0 }, size: CGSize { width: 10.0, height: 10.0 } })];

        () = msg![env; parent addSubview:child1];
        () = msg![env; parent addSubview:child2];
        () = msg![env; parent addSubview:child3];

        let subviews: id = msg![env; parent subviews];
        let subviews_count: NSUInteger = msg![env; subviews count];
        assert_eq!(subviews_count, 3);

        // Immutable NSArray of marks (_touchHLE_NSArray created via [NSArray arrayWithArray:marks_mut])
        let marks_mut: id = msg_class![env; NSMutableArray array];
        () = msg![env; marks_mut addObject:child1];
        () = msg![env; marks_mut addObject:child2];
        let marks: id = msg_class![env; NSArray arrayWithArray:marks_mut];
        let rem_sel: SEL = env.objc.register_host_selector("removeFromSuperview".to_string(), &mut env.mem);

        () = msg![env; marks makeObjectsPerformSelector:rem_sel];

        let child1_super: id = msg![env; child1 superview];
        let child2_super: id = msg![env; child2 superview];
        let child3_super: id = msg![env; child3 superview];
        assert_eq!(child1_super, nil);
        assert_eq!(child2_super, nil);
        assert_eq!(child3_super, parent);

        let remaining_subviews: id = msg![env; parent subviews];
        let remaining_count: NSUInteger = msg![env; remaining_subviews count];
        assert_eq!(remaining_count, 1);
        let remaining_first: id = msg![env; remaining_subviews objectAtIndex:0u32];
        assert_eq!(remaining_first, child3);

        // Mutable NSMutableArray of marks (_touchHLE_NSMutableArray)
        let mut_arr_marks: id = msg_class![env; NSMutableArray arrayWithObject:child3];
        () = msg![env; mut_arr_marks makeObjectsPerformSelector:rem_sel];
        let child3_super_after: id = msg![env; child3 superview];
        assert_eq!(child3_super_after, nil);
        let after_mut_subviews: id = msg![env; parent subviews];
        let after_mut_count: NSUInteger = msg![env; after_mut_subviews count];
        assert_eq!(after_mut_count, 0u32);
    }

    #[test]
    fn test_array_make_objects_perform_selector_with_object() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        let inner1: id = msg_class![env; NSMutableArray array];
        let inner2: id = msg_class![env; NSMutableArray array];

        let outer_mut: id = msg_class![env; NSMutableArray array];
        () = msg![env; outer_mut addObject:inner1];
        () = msg![env; outer_mut addObject:inner2];
        let outer_imm: id = msg_class![env; NSArray arrayWithArray:outer_mut];

        let test_val: id = msg_class![env; NSNumber numberWithInt:42];
        let add_sel: SEL = env.objc.register_host_selector("addObject:".to_string(), &mut env.mem);

        // Test on immutable NSArray (_touchHLE_NSArray)
        () = msg![env; outer_imm makeObjectsPerformSelector:add_sel withObject:test_val];
        let inner1_count: NSUInteger = msg![env; inner1 count];
        assert_eq!(inner1_count, 1u32);
        let inner1_obj0: id = msg![env; inner1 objectAtIndex:0u32];
        assert_eq!(inner1_obj0, test_val);
        let inner2_count: NSUInteger = msg![env; inner2 count];
        assert_eq!(inner2_count, 1u32);
        let inner2_obj0: id = msg![env; inner2 objectAtIndex:0u32];
        assert_eq!(inner2_obj0, test_val);

        // Test on mutable NSMutableArray (_touchHLE_NSMutableArray)
        let test_val2: id = msg_class![env; NSNumber numberWithInt:99];
        () = msg![env; outer_mut makeObjectsPerformSelector:add_sel withObject:test_val2];
        let inner1_count2: NSUInteger = msg![env; inner1 count];
        assert_eq!(inner1_count2, 2u32);
        let inner1_obj1: id = msg![env; inner1 objectAtIndex:1u32];
        assert_eq!(inner1_obj1, test_val2);
        let inner2_count2: NSUInteger = msg![env; inner2 count];
        assert_eq!(inner2_count2, 2u32);
        let inner2_obj1: id = msg![env; inner2 objectAtIndex:1u32];
        assert_eq!(inner2_obj1, test_val2);
    }

    #[test]
    fn test_set_make_objects_perform_selector() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        let n1: id = msg_class![env; NSNumber numberWithInt:1];
        let n2: id = msg_class![env; NSNumber numberWithInt:2];
        let s1: id = msg_class![env; NSMutableArray array];
        () = msg![env; s1 addObject:n1];
        let s2: id = msg_class![env; NSMutableArray array];
        () = msg![env; s2 addObject:n2];
        let add_sel: SEL = env.objc.register_host_selector("addObject:".to_string(), &mut env.mem);
        let clear_sel: SEL = env.objc.register_host_selector("removeAllObjects".to_string(), &mut env.mem);
        let val: id = msg_class![env; NSNumber numberWithInt:100];

        let arr: id = msg_class![env; NSMutableArray array];
        () = msg![env; arr addObject:s1];
        () = msg![env; arr addObject:s2];

        // 1. NSSet (_touchHLE_NSSet)
        let set_imm: id = msg_class![env; NSSet setWithArray:arr];
        () = msg![env; set_imm makeObjectsPerformSelector:add_sel withObject:val];
        let s1_cnt: NSUInteger = msg![env; s1 count];
        assert_eq!(s1_cnt, 2u32);
        let s2_cnt: NSUInteger = msg![env; s2 count];
        assert_eq!(s2_cnt, 2u32);
        () = msg![env; set_imm makeObjectsPerformSelector:clear_sel];
        let s1_cnt0: NSUInteger = msg![env; s1 count];
        assert_eq!(s1_cnt0, 0u32);
        let s2_cnt0: NSUInteger = msg![env; s2 count];
        assert_eq!(s2_cnt0, 0u32);

        // 2. NSMutableSet (_touchHLE_NSMutableSet)
        let m1: id = msg_class![env; NSMutableArray array];
        () = msg![env; m1 addObject:n1];
        let m2: id = msg_class![env; NSMutableArray array];
        () = msg![env; m2 addObject:n2];
        let arr_m: id = msg_class![env; NSMutableArray array];
        () = msg![env; arr_m addObject:m1];
        () = msg![env; arr_m addObject:m2];
        let mset: id = msg_class![env; NSMutableSet setWithArray:arr_m];
        () = msg![env; mset makeObjectsPerformSelector:add_sel withObject:val];
        let m1_cnt: NSUInteger = msg![env; m1 count];
        assert_eq!(m1_cnt, 2u32);
        let m2_cnt: NSUInteger = msg![env; m2 count];
        assert_eq!(m2_cnt, 2u32);
        () = msg![env; mset makeObjectsPerformSelector:clear_sel];
        let m1_cnt0: NSUInteger = msg![env; m1 count];
        assert_eq!(m1_cnt0, 0u32);
        let m2_cnt0: NSUInteger = msg![env; m2 count];
        assert_eq!(m2_cnt0, 0u32);

        // 3. NSCountedSet (_touchHLE_NSCountedSet)
        let c1: id = msg_class![env; NSMutableArray array];
        () = msg![env; c1 addObject:n1];
        let c2: id = msg_class![env; NSMutableArray array];
        () = msg![env; c2 addObject:n2];
        let cset: id = msg_class![env; NSCountedSet set];
        () = msg![env; cset addObject:c1];
        () = msg![env; cset addObject:c2];
        () = msg![env; cset makeObjectsPerformSelector:add_sel withObject:val];
        let c1_cnt: NSUInteger = msg![env; c1 count];
        assert_eq!(c1_cnt, 2u32);
        let c2_cnt: NSUInteger = msg![env; c2 count];
        assert_eq!(c2_cnt, 2u32);
        () = msg![env; cset makeObjectsPerformSelector:clear_sel];
        let c1_cnt0: NSUInteger = msg![env; c1 count];
        assert_eq!(c1_cnt0, 0u32);
        let c2_cnt0: NSUInteger = msg![env; c2 count];
        assert_eq!(c2_cnt0, 0u32);
    }

    #[test]
    fn test_array_subarray_and_add_object_and_sort() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        let n1: id = msg_class![env; NSNumber numberWithInt:1];
        let n2: id = msg_class![env; NSNumber numberWithInt:2];
        let n3: id = msg_class![env; NSNumber numberWithInt:3];

        let mut_arr: id = msg_class![env; NSMutableArray array];
        () = msg![env; mut_arr addObject:n1];
        () = msg![env; mut_arr addObject:n2];
        () = msg![env; mut_arr addObject:n3];
        let arr: id = msg_class![env; NSArray arrayWithArray:mut_arr];

        // subarrayWithRange on NSArray
        let sub: id = msg![env; arr subarrayWithRange:(NSRange { location: 1, length: 2 })];
        let sub_cnt: NSUInteger = msg![env; sub count];
        assert_eq!(sub_cnt, 2u32);
        let sub_0: id = msg![env; sub objectAtIndex:0u32];
        assert_eq!(sub_0, n2);
        let sub_1: id = msg![env; sub objectAtIndex:1u32];
        assert_eq!(sub_1, n3);

        let sub_empty: id = msg![env; arr subarrayWithRange:(NSRange { location: 1, length: 0 })];
        let sub_empty_cnt: NSUInteger = msg![env; sub_empty count];
        assert_eq!(sub_empty_cnt, 0u32);

        // subarrayWithRange on NSMutableArray
        let sub_mut: id = msg![env; mut_arr subarrayWithRange:(NSRange { location: 0, length: 2 })];
        let sub_mut_cnt: NSUInteger = msg![env; sub_mut count];
        assert_eq!(sub_mut_cnt, 2u32);
        let sub_mut_0: id = msg![env; sub_mut objectAtIndex:0u32];
        assert_eq!(sub_mut_0, n1);
        let sub_mut_1: id = msg![env; sub_mut objectAtIndex:1u32];
        assert_eq!(sub_mut_1, n2);

        // arrayByAddingObject on NSArray
        let n4: id = msg_class![env; NSNumber numberWithInt:4];
        let arr_added: id = msg![env; arr arrayByAddingObject:n4];
        let arr_added_cnt: NSUInteger = msg![env; arr_added count];
        assert_eq!(arr_added_cnt, 4u32);
        let arr_added_3: id = msg![env; arr_added objectAtIndex:3u32];
        assert_eq!(arr_added_3, n4);
        let arr_cnt: NSUInteger = msg![env; arr count];
        assert_eq!(arr_cnt, 3u32);

        // arrayByAddingObject on NSMutableArray
        let mut_added: id = msg![env; mut_arr arrayByAddingObject:n4];
        let mut_added_cnt: NSUInteger = msg![env; mut_added count];
        assert_eq!(mut_added_cnt, 4u32);
        let mut_added_3: id = msg![env; mut_added objectAtIndex:3u32];
        assert_eq!(mut_added_3, n4);
        let mut_arr_cnt: NSUInteger = msg![env; mut_arr count];
        assert_eq!(mut_arr_cnt, 3u32);

        // arrayByAddingObjectsFromArray on NSArray and NSMutableArray
        let extra_mut: id = msg_class![env; NSMutableArray array];
        let n5: id = msg_class![env; NSNumber numberWithInt:5];
        () = msg![env; extra_mut addObject:n5];
        let arr_added_many: id = msg![env; arr arrayByAddingObjectsFromArray:extra_mut];
        let arr_added_many_cnt: NSUInteger = msg![env; arr_added_many count];
        assert_eq!(arr_added_many_cnt, 4u32);
        let arr_added_many_3: id = msg![env; arr_added_many objectAtIndex:3u32];
        assert_eq!(arr_added_many_3, n5);

        let mut_added_many: id = msg![env; mut_arr arrayByAddingObjectsFromArray:extra_mut];
        let mut_added_many_cnt: NSUInteger = msg![env; mut_added_many count];
        assert_eq!(mut_added_many_cnt, 4u32);
        let mut_added_many_3: id = msg![env; mut_added_many objectAtIndex:3u32];
        assert_eq!(mut_added_many_3, n5);
        // Ensure returning array is immutable _touchHLE_NSArray
        let imm_class = env.objc.get_known_class("_touchHLE_NSArray", &mut env.mem);
        let mut_added_class: Class = msg![env; mut_added_many class];
        assert_eq!(mut_added_class, imm_class);

        // sortedArrayUsingSelector on NSArray and NSMutableArray
        let cmp_sel: SEL = env.objc.register_host_selector("compare:".to_string(), &mut env.mem);
        let unsorted_mut: id = msg_class![env; NSMutableArray array];
        () = msg![env; unsorted_mut addObject:n3];
        () = msg![env; unsorted_mut addObject:n1];
        () = msg![env; unsorted_mut addObject:n2];
        let unsorted_arr: id = msg_class![env; NSArray arrayWithArray:unsorted_mut];

        let sorted_arr: id = msg![env; unsorted_arr sortedArrayUsingSelector:cmp_sel];
        let sorted_arr_cnt: NSUInteger = msg![env; sorted_arr count];
        assert_eq!(sorted_arr_cnt, 3u32);
        let sorted_arr_0: id = msg![env; sorted_arr objectAtIndex:0u32];
        assert_eq!(sorted_arr_0, n1);
        let sorted_arr_1: id = msg![env; sorted_arr objectAtIndex:1u32];
        assert_eq!(sorted_arr_1, n2);
        let sorted_arr_2: id = msg![env; sorted_arr objectAtIndex:2u32];
        assert_eq!(sorted_arr_2, n3);

        let sorted_mut: id = msg![env; unsorted_mut sortedArrayUsingSelector:cmp_sel];
        let sorted_mut_cnt: NSUInteger = msg![env; sorted_mut count];
        assert_eq!(sorted_mut_cnt, 3u32);
        let sorted_mut_0: id = msg![env; sorted_mut objectAtIndex:0u32];
        assert_eq!(sorted_mut_0, n1);
        let sorted_mut_1: id = msg![env; sorted_mut objectAtIndex:1u32];
        assert_eq!(sorted_mut_1, n2);
        let sorted_mut_2: id = msg![env; sorted_mut objectAtIndex:2u32];
        assert_eq!(sorted_mut_2, n3);
    }

    #[test]
    fn test_dictionary_all_keys_for_object() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        let k1 = ns_string::from_rust_string(env, "k1".to_string());
        let k2 = ns_string::from_rust_string(env, "k2".to_string());
        let k3 = ns_string::from_rust_string(env, "k3".to_string());
        let target_val = ns_string::from_rust_string(env, "target".to_string());
        let other_val = ns_string::from_rust_string(env, "other".to_string());

        // 1. Immutable NSDictionary
        let dict_imm = super::super::ns_dictionary::dict_from_keys_and_objects(
            env,
            &[(k1, target_val), (k2, other_val), (k3, target_val)],
        );

        let matched_keys: id = msg![env; dict_imm allKeysForObject:target_val];
        let matched_count: NSUInteger = msg![env; matched_keys count];
        assert_eq!(matched_count, 2u32);
        let has_k1: bool = msg![env; matched_keys containsObject:k1];
        let has_k3: bool = msg![env; matched_keys containsObject:k3];
        assert!(has_k1);
        assert!(has_k3);

        let non_existent = ns_string::from_rust_string(env, "nonexistent".to_string());
        let unmatched_keys: id = msg![env; dict_imm allKeysForObject:non_existent];
        let unmatched_count: NSUInteger = msg![env; unmatched_keys count];
        assert_eq!(unmatched_count, 0u32);

        // Passing nil to allKeysForObject returns empty array
        let nil_keys: id = msg![env; dict_imm allKeysForObject:nil];
        let nil_keys_cnt: NSUInteger = msg![env; nil_keys count];
        assert_eq!(nil_keys_cnt, 0u32);

        // 2. NSMutableDictionary
        let dict_mut = super::super::ns_dictionary::mutable_dict_from_keys_and_objects(
            env,
            &[(k1, target_val), (k2, other_val), (k3, target_val)],
        );
        let mut_matched: id = msg![env; dict_mut allKeysForObject:target_val];
        let mut_matched_count: NSUInteger = msg![env; mut_matched count];
        assert_eq!(mut_matched_count, 2u32);
        let mut_has_k1: bool = msg![env; mut_matched containsObject:k1];
        let mut_has_k3: bool = msg![env; mut_matched containsObject:k3];
        assert!(mut_has_k1);
        assert!(mut_has_k3);

        let mut_nil_keys: id = msg![env; dict_mut allKeysForObject:nil];
        let mut_nil_keys_cnt: NSUInteger = msg![env; mut_nil_keys count];
        assert_eq!(mut_nil_keys_cnt, 0u32);
    }

    #[test]
    fn test_array_make_objects_perform_selector_mutation_safety() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        // Verify that if an element in the array mutates or clears the receiver array,
        // snapshotting prevents index-out-of-bounds panics or iteration corruption.
        let target_arr: id = msg_class![env; NSMutableArray array];
        let clear_sel: SEL = env.objc.register_host_selector("removeAllObjects".to_string(), &mut env.mem);

        // The array contains two references to target_arr
        () = msg![env; target_arr addObject:target_arr];
        () = msg![env; target_arr addObject:target_arr];
        let target_arr_cnt: NSUInteger = msg![env; target_arr count];
        assert_eq!(target_arr_cnt, 2u32);

        // Performing removeAllObjects during makeObjectsPerformSelector:
        // On iteration 0, target_arr removeAllObjects empties target_arr.
        // On iteration 1, snapshotting retains target_arr, so iteration 1 runs safely without panic.
        () = msg![env; target_arr makeObjectsPerformSelector:clear_sel];
        let target_arr_cnt0: NSUInteger = msg![env; target_arr count];
        assert_eq!(target_arr_cnt0, 0u32);
    }

    #[test]
    fn test_set_all_objects_and_any_object() {
        let (_guard, mut env_inst) = get_test_environment();
        let env = &mut env_inst;

        let n1: id = msg_class![env; NSNumber numberWithInt:10];
        let n2: id = msg_class![env; NSNumber numberWithInt:20];

        let arr: id = msg_class![env; NSMutableArray array];
        () = msg![env; arr addObject:n1];
        () = msg![env; arr addObject:n2];

        let s: id = msg_class![env; NSSet setWithArray:arr];
        let all: id = msg![env; s allObjects];
        let all_cnt: NSUInteger = msg![env; all count];
        assert_eq!(all_cnt, 2u32);
        let any: id = msg![env; s anyObject];
        assert!(any == n1 || any == n2);

        let ms: id = msg_class![env; NSMutableSet setWithArray:arr];
        let ms_all: id = msg![env; ms allObjects];
        let ms_all_cnt: NSUInteger = msg![env; ms_all count];
        assert_eq!(ms_all_cnt, 2u32);
        let ms_any: id = msg![env; ms anyObject];
        assert!(ms_any == n1 || ms_any == n2);
    }
}
