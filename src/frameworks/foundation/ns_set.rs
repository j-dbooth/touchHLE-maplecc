/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The `NSSet` class cluster, including `NSMutableSet` and `NSCountedSet`.

use super::ns_array;
use super::ns_dictionary::DictionaryHostObject;
use super::ns_enumerator::{fast_enumeration_helper, NSFastEnumerationState};
use super::ns_keyed_archiver::{encode_object, get_value_to_encode_for_current_key};
use super::{_nib_archive_decoder, ns_keyed_unarchiver, NSUInteger};
use crate::abi::DotDotDot;
use crate::environment::Environment;
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, Class, ClassExports,
    HostObject, NSZonePtr, SEL,
};
use std::collections::HashMap;

type Hash = NSUInteger;

/// Belongs to _touchHLE_NSSet
#[derive(Debug, Default)]
struct SetHostObject {
    dict: DictionaryHostObject,
}
impl HostObject for SetHostObject {}

/// Belongs to _touchHLE_NSCountedSet
#[derive(Debug, Default)]
struct CountedSetHostObject {
    map: HashMap<Hash, Vec<(id, usize)>>,
    count: NSUInteger,
}
impl HostObject for CountedSetHostObject {}

impl CountedSetHostObject {
    fn lookup(&self, env: &mut Environment, key: id) -> usize {
        let hash: Hash = msg![env; key hash];
        let Some(collisions) = self.map.get(&hash) else {
            return 0;
        };
        for &(candidate_key, count) in collisions {
            if candidate_key == key || msg![env; candidate_key isEqual:key] {
                return count;
            }
        }
        0
    }

    fn find_key(&self, env: &mut Environment, key: id) -> id {
        let hash: Hash = msg![env; key hash];
        let Some(collisions) = self.map.get(&hash) else {
            return nil;
        };
        for &(candidate_key, _) in collisions {
            if candidate_key == key || msg![env; candidate_key isEqual:key] {
                return candidate_key;
            }
        }
        nil
    }

    fn add(&mut self, env: &mut Environment, key: id) {
        let hash: Hash = msg![env; key hash];
        if let Some(collisions) = self.map.get_mut(&hash) {
            for &mut (candidate_key, ref mut count) in collisions.iter_mut() {
                if candidate_key == key || msg![env; candidate_key isEqual:key] {
                    *count += 1;
                    return;
                }
            }
            let key = retain(env, key);
            collisions.push((key, 1));
            self.count += 1;
        } else {
            let key = retain(env, key);
            self.map.insert(hash, vec![(key, 1)]);
            self.count += 1;
        }
    }

    fn remove(&mut self, env: &mut Environment, key: id) {
        let hash: Hash = msg![env; key hash];
        let Some(collisions) = self.map.get_mut(&hash) else {
            return;
        };
        let Some(idx) = collisions.iter().position(|&(candidate_key, _)| {
            candidate_key == key || msg![env; candidate_key isEqual:key]
        }) else {
            return;
        };
        collisions[idx].1 -= 1;
        if collisions[idx].1 == 0 {
            let (existing_key, _) = collisions.remove(idx);
            release(env, existing_key);
            self.count -= 1;
        }
    }

    fn release(&mut self, env: &mut Environment) {
        for collisions in self.map.values() {
            for &(key, _) in collisions {
                release(env, key);
            }
        }
    }

    fn iter_keys(&self) -> impl Iterator<Item = id> + '_ {
        self.map.values().flatten().map(|&(key, _)| key)
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// NSSet is an abstract class. A subclass must provide:
// - (NSUInteger)count;
// - (id)member:(id)object;
// - (NSEnumerator*)objectEnumerator;
// We can pick whichever subclass we want for the various alloc methods.
// For the time being, that will always be _touchHLE_NSSet.
@implementation NSSet: NSObject

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    msg_class![env; _touchHLE_NSSet allocWithZone:zone]
}

+ (id)set {
    let set: id = msg![env; this new];
    autorelease(env, set)
}

+ (id)setWithArray:(id)array { // NSArray *
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithArray:array];
    autorelease(env, new)
}

+ (id)setWithObject:(id)object {
    assert!(object != nil);
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithObject:object];
    autorelease(env, new)
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

+ (id)setWithSet:(id)set {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithSet:set];
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)zone {
    let all_objects: id = msg![env; this allObjects];
    let new: id = msg_class![env; NSMutableSet allocWithZone:zone];
    msg![env; new initWithArray:all_objects]
}

- (id)member:(id)object {
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            return nil;
        }
        if msg![env; next isEqual:object] {
            return next;
        }
    }
}

- (bool)containsObject:(id)object {
    let member: id = msg![env; this member:object];
    member != nil
}

- (NSUInteger)hash {
    let count: NSUInteger = msg![env; this count];
    count
}

- (bool)isEqualToSet:(id)other {
    if other == this {
        return true;
    }
    if other == nil {
        return false;
    }
    let count1: NSUInteger = msg![env; this count];
    let count2: NSUInteger = msg![env; other count];
    if count1 != count2 {
        return false;
    }
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        let contains: bool = msg![env; other containsObject:next];
        if !contains {
            return false;
        }
    }
    true
}

- (bool)isEqual:(id)other {
    let set_class = env.objc.get_known_class("NSSet", &mut env.mem);
    let other_class: Class = msg![env; other class];
    if env.objc.class_is_subclass_of(other_class, set_class) {
        msg![env; this isEqualToSet:other]
    } else {
        false
    }
}

- (id)allObjects {
    let mut objects = Vec::new();
    let enumerator: id = msg![env; this objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        retain(env, next);
        objects.push(next);
    }
    let res = ns_array::from_vec(env, objects);
    autorelease(env, res)
}

- (id)anyObject {
    let enumerator: id = msg![env; this objectEnumerator];
    msg![env; enumerator nextObject]
}

- (())makeObjectsPerformSelector:(SEL)sel {
    let all_objects: id = msg![env; this allObjects];
    () = msg![env; all_objects makeObjectsPerformSelector:sel];
}

- (())makeObjectsPerformSelector:(SEL)sel withObject:(id)argument {
    let all_objects: id = msg![env; this allObjects];
    () = msg![env; all_objects makeObjectsPerformSelector:sel withObject:argument];
}

@end

// NSMutableSet is an abstract class. A subclass must provide everything
// NSSet provides, plus:
// - (void)addObject:(id)object;
// - (void)removeObject:(id)object;
// Note that it inherits from NSSet, so we must ensure we override any default
// methods that would be inappropriate for mutability.
@implementation NSMutableSet: NSSet

+ (id)allocWithZone:(NSZonePtr)zone {
    // NSSet might be subclassed by something which needs allocWithZone:
    // to have the normal behaviour. Unimplemented: call superclass alloc then.
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    msg_class![env; _touchHLE_NSMutableSet allocWithZone:zone]
}

+ (id)set {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    autorelease(env, new)
}

+ (id)setWithArray:(id)array {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithArray:array];
    autorelease(env, new)
}

+ (id)setWithSet:(id)set {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithSet:set];
    autorelease(env, new)
}

+ (id)setWithObjects:(id)first_obj, ...args {
    assert!(this == env.objc.get_known_class("NSMutableSet", &mut env.mem));
    let new: id = msg![env; this alloc];
    env.objc.borrow_mut::<SetHostObject>(new).dict = set_from_objects(env, first_obj, args);
    autorelease(env, new)
}

+ (id)setWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

// NSCopying implementation
- (id)copyWithZone:(NSZonePtr)zone {
    let all_objects: id = msg![env; this allObjects];
    let new: id = msg_class![env; NSSet allocWithZone:zone];
    msg![env; new initWithArray:all_objects]
}

// NSMutableCopying implementation
- (id)mutableCopyWithZone:(NSZonePtr)zone {
    let all_objects: id = msg![env; this allObjects];
    let new: id = msg_class![env; NSMutableSet allocWithZone:zone];
    msg![env; new initWithArray:all_objects]
}

- (())addObjectsFromArray:(id)array {
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let obj: id = msg![env; array objectAtIndex:i];
        () = msg![env; this addObject:obj];
    }
}

- (())minusSet:(id)other {
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this removeObject:next];
    }
}

- (())intersectSet:(id)other {
    let all_objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; all_objects count];
    for i in 0..count {
        let obj: id = msg![env; all_objects objectAtIndex:i];
        let contains: bool = msg![env; other containsObject:obj];
        if !contains {
            () = msg![env; this removeObject:obj];
        }
    }
}

- (())setSet:(id)other {
    () = msg![env; this removeAllObjects];
    () = msg![env; this unionSet:other];
}

@end

// Our private subclass that is the single implementation of NSSet for the
// time being.
@implementation _touchHLE_NSSet: NSSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (id)initWithSet:(id)other_set {
    let all_objects: id = msg![env; other_set allObjects];
    msg![env; this initWithArray:all_objects]
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: more init methods, etc

// TODO: accessors
- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)member:(id)object {
    if object == nil {
        return nil;
    }
    let host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    let found = host_obj.dict.find_key(env, object);
    *env.objc.borrow_mut(this) = host_obj;
    found
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects: Vec<id> = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().collect();
    for &obj in &objects {
        retain(env, obj);
    }
    let res = ns_array::from_vec(env, objects);
    autorelease(env, res)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

@end

// Our private subclass that is the single implementation of NSMutableSet for
// the time being.
@implementation _touchHLE_NSMutableSet: NSMutableSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SetHostObject {
        dict: Default::default(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithObject:(id)object {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, object, null, /* copy_key: */ false);

    env.objc.borrow_mut::<SetHostObject>(this).dict = dict;

    this
}

- (id)initWithObjects:(id)first_obj, ...args {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_objects(env, first_obj, args);
    this
}

- (id)initWithArray:(id)array {
    env.objc.borrow_mut::<SetHostObject>(this).dict = set_from_array(env, array);
    this
}

- (id)initWithSet:(id)other_set {
    let all_objects: id = msg![env; other_set allObjects];
    msg![env; this initWithArray:all_objects]
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    encode_with_coder_inner(env, this, coder)
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    // TODO: capacity
    msg![env; this init]
}

- (())dealloc {
    std::mem::take(&mut env.objc.borrow_mut::<SetHostObject>(this).dict).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: init methods etc

- (NSUInteger)count {
    env.objc.borrow_mut::<SetHostObject>(this).dict.count
}

- (id)member:(id)object {
    if object == nil {
        return nil;
    }
    let host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    let found = host_obj.dict.find_key(env, object);
    *env.objc.borrow_mut(this) = host_obj;
    found
}

- (id)anyObject {
    let object_or_none = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().next();
    match object_or_none {
        Some(object) => object,
        None => nil
    }
}

- (id)allObjects {
    let objects: Vec<id> = env.objc.borrow_mut::<SetHostObject>(this).dict.iter_keys().collect();
    for &obj in &objects {
        retain(env, obj);
    }
    let res = ns_array::from_vec(env, objects);
    autorelease(env, res)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

// NSFastEnumeration implementation
- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    // TODO: check that set wasn't mutated!
    // We assume that order in which objects are reported is consistent
    // between calls!
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

// TODO: more mutation methods

- (())addObject:(id)object {
    let null: id = msg_class![env; NSNull null];
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.insert(env, object, null, /* copy_key: */ false);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeObject:(id)object {
    let mut host_obj: SetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.dict.remove(env, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeAllObjects {
    let mut old_host_obj = std::mem::replace(
        env.objc.borrow_mut(this),
        SetHostObject {
            dict: Default::default(),
        },
    );
    old_host_obj.dict.release(env);
}

- (())unionSet:(id)other { // NSSet *
    let enumerator: id = msg![env; other objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
}

@end

@implementation NSCountedSet: NSMutableSet

+ (id)allocWithZone:(NSZonePtr)zone {
    assert!(this == env.objc.get_known_class("NSCountedSet", &mut env.mem));
    msg_class![env; _touchHLE_NSCountedSet allocWithZone:zone]
}

+ (id)set {
    let set: id = msg![env; this new];
    autorelease(env, set)
}

+ (id)setWithCapacity:(NSUInteger)capacity {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCapacity:capacity];
    autorelease(env, new)
}

+ (id)setWithArray:(id)array {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithArray:array];
    autorelease(env, new)
}

+ (id)setWithSet:(id)other {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithSet:other];
    autorelease(env, new)
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    todo!("TODO: Implement [NSCountedSet initWithCapacity:] for custom subclasses")
}

- (id)initWithSet:(id)_set {
    todo!("TODO: Implement [NSCountedSet initWithSet:] for custom subclasses")
}

- (NSUInteger)countForObject:(id)_object {
    todo!("TODO: Implement [NSCountedSet countForObject:] for custom subclasses")
}

@end

@implementation _touchHLE_NSCountedSet: NSCountedSet

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<CountedSetHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    *env.objc.borrow_mut(this) = <CountedSetHostObject as Default>::default();
    this
}

- (id)initWithCapacity:(NSUInteger)_capacity {
    *env.objc.borrow_mut(this) = <CountedSetHostObject as Default>::default();
    this
}

- (id)initWithArray:(id)array {
    *env.objc.borrow_mut(this) = <CountedSetHostObject as Default>::default();
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let obj: id = msg![env; array objectAtIndex:i];
        () = msg![env; this addObject:obj];
    }
    this
}

- (id)initWithSet:(id)other_set {
    *env.objc.borrow_mut(this) = <CountedSetHostObject as Default>::default();
    let enumerator: id = msg![env; other_set objectEnumerator];
    loop {
        let next: id = msg![env; enumerator nextObject];
        if next == nil {
            break;
        }
        () = msg![env; this addObject:next];
    }
    this
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    counted_set_init_with_coder_inner(env, this, coder)
}
- (())encodeWithCoder:(id)coder {
    counted_set_encode_with_coder_inner(env, this, coder)
}

- (())dealloc {
    std::mem::take(env.objc.borrow_mut::<CountedSetHostObject>(this)).release(env);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (NSUInteger)count {
    env.objc.borrow::<CountedSetHostObject>(this).count
}

- (NSUInteger)countForObject:(id)object {
    let host_obj: CountedSetHostObject = std::mem::take(env.objc.borrow_mut(this));
    let count = host_obj.lookup(env, object) as NSUInteger;
    *env.objc.borrow_mut(this) = host_obj;
    count
}

- (())addObject:(id)object {
    assert!(object != nil);
    let mut host_obj: CountedSetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.add(env, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeObject:(id)object {
    assert!(object != nil);
    let mut host_obj: CountedSetHostObject = std::mem::take(env.objc.borrow_mut(this));
    host_obj.remove(env, object);
    *env.objc.borrow_mut(this) = host_obj;
}

- (())removeAllObjects {
    let mut old = std::mem::take(env.objc.borrow_mut::<CountedSetHostObject>(this));
    old.release(env);
}

- (id)anyObject {
    env.objc.borrow::<CountedSetHostObject>(this).iter_keys().next().unwrap_or(nil)
}

- (id)allObjects {
    let objects: Vec<id> = env.objc.borrow::<CountedSetHostObject>(this).iter_keys().collect();
    for &obj in &objects {
        retain(env, obj);
    }
    let res = ns_array::from_vec(env, objects);
    autorelease(env, res)
}

- (id)objectEnumerator { // NSEnumerator*
    let array: id = msg![env; this allObjects];
    msg![env; array objectEnumerator]
}

- (id)member:(id)object {
    if object == nil {
        return nil;
    }
    let host_obj: CountedSetHostObject = std::mem::take(env.objc.borrow_mut(this));
    let found = host_obj.find_key(env, object);
    *env.objc.borrow_mut(this) = host_obj;
    found
}

- (bool)containsObject:(id)object {
    let member: id = msg![env; this member:object];
    member != nil
}

- (NSUInteger)countByEnumeratingWithState:(MutPtr<NSFastEnumerationState>)state
                                  objects:(MutPtr<id>)stackbuf
                                    count:(NSUInteger)len {
    let objects: id = msg![env; this allObjects];
    let count: NSUInteger = msg![env; objects count];
    fast_enumeration_helper(env, this, |env, idx| {
        if idx < count {
            msg![env; objects objectAtIndex:idx]
        } else {
            nil
        }
    }, state, stackbuf, len)
}

@end

};

/// Helper method shared between `initWithObjects:` of `_touchHLE_NSSet` and
/// `_touchHLE_NSMutableSet`
fn set_from_objects(env: &mut Environment, first_obj: id, args: DotDotDot) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    dict.insert(env, first_obj, null, /* copy_key: */ false);
    let mut varargs = args.start();
    loop {
        let next_arg: id = varargs.next(env);
        if next_arg == nil {
            break;
        }
        dict.insert(env, next_arg, null, /* copy_key: */ false);
    }
    dict
}

/// Helper method shared between `initWithArray:` of `_touchHLE_NSSet` and
/// `_touchHLE_NSMutableSet`
fn set_from_array(env: &mut Environment, array: id) -> DictionaryHostObject {
    let null: id = msg_class![env; NSNull null];

    let mut dict = <DictionaryHostObject as Default>::default();
    let count: NSUInteger = msg![env; array count];
    for i in 0..count {
        let next: id = msg![env; array objectAtIndex:i];
        dict.insert(env, next, null, /* copy_key: */ false);
    }
    dict
}

fn encode_with_coder_inner(env: &mut Environment, set: id, coder: id) {
    let keys: Vec<id> = env.objc.borrow::<SetHostObject>(set).dict.iter_keys().collect();
    let mut encoded_vals = vec![];
    for v in keys {
        let vv = encode_object(env, coder, v);
        encoded_vals.push(plist::Value::Uid(vv));
    }

    let scope = get_value_to_encode_for_current_key(env, coder);
    scope.insert("NS.objects".to_string(), plist::Value::Array(encoded_vals));
}

fn init_with_coder_inner(env: &mut Environment, set: id, coder: id) -> id {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _touchHLE_NIBArchiveDecoder class];
    let objects = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_array(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_array(env, coder)
    } else {
        unimplemented!()
    };

    let null: id = msg_class![env; NSNull null];
    let mut dict = <DictionaryHostObject as Default>::default();
    for &obj in &objects {
        dict.insert(env, obj, null, /* copy_key: */ false);
        release(env, obj);
    }

    let host_object: &mut SetHostObject = env.objc.borrow_mut(set);
    assert!(host_object.dict.count == 0);
    host_object.dict = dict;
    set
}

fn counted_set_encode_with_coder_inner(env: &mut Environment, set: id, coder: id) {
    let keys: Vec<id> = env.objc.borrow::<CountedSetHostObject>(set).iter_keys().collect();
    let mut encoded_vals = vec![];
    for v in keys {
        let vv = encode_object(env, coder, v);
        encoded_vals.push(plist::Value::Uid(vv));
    }

    let scope = get_value_to_encode_for_current_key(env, coder);
    scope.insert("NS.objects".to_string(), plist::Value::Array(encoded_vals));
}

fn counted_set_init_with_coder_inner(env: &mut Environment, set: id, coder: id) -> id {
    let class: Class = msg![env; coder class];
    let keyed_unarch_class: Class = msg_class![env; NSKeyedUnarchiver class];
    let nib_archive_class: Class = msg_class![env; _touchHLE_NIBArchiveDecoder class];
    let objects = if env.objc.class_is_subclass_of(class, keyed_unarch_class) {
        ns_keyed_unarchiver::decode_current_array(env, coder)
    } else if env.objc.class_is_subclass_of(class, nib_archive_class) {
        _nib_archive_decoder::decode_current_array(env, coder)
    } else {
        unimplemented!()
    };

    let mut host_obj = <CountedSetHostObject as Default>::default();
    for &obj in &objects {
        host_obj.add(env, obj);
        release(env, obj);
    }

    *env.objc.borrow_mut::<CountedSetHostObject>(set) = host_obj;
    set
}
