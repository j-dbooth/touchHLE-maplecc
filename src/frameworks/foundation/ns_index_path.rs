/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSIndexPath`.

use super::{NSComparisonResult, NSInteger, NSNotFound, NSOrderedAscending, NSOrderedDescending, NSOrderedSame, NSUInteger};
use crate::mem::ConstPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, objc_classes, retain, Class, ClassExports, HostObject,
    NSZonePtr,
};

struct NSIndexPathHostObject {
    indexes: Vec<NSUInteger>,
}
impl HostObject for NSIndexPathHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSIndexPath: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSIndexPathHostObject {
        indexes: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)indexPathWithIndex:(NSUInteger)index {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithIndex:index];
    autorelease(env, new)
}

+ (id)indexPathWithIndexes:(ConstPtr<NSUInteger>)indexes length:(NSUInteger)length {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithIndexes:indexes length:length];
    autorelease(env, new)
}

+ (id)indexPathForRow:(NSInteger)row inSection:(NSInteger)section {
    let new: id = msg![env; this alloc];
    let host = env.objc.borrow_mut::<NSIndexPathHostObject>(new);
    host.indexes = vec![section as NSUInteger, row as NSUInteger];
    autorelease(env, new)
}

- (id)initWithIndex:(NSUInteger)index {
    let host = env.objc.borrow_mut::<NSIndexPathHostObject>(this);
    host.indexes = vec![index];
    this
}

- (id)initWithIndexes:(ConstPtr<NSUInteger>)indexes length:(NSUInteger)length {
    let mut vec = Vec::with_capacity(length as usize);
    for i in 0..length {
        vec.push(env.mem.read(indexes + i));
    }
    let host = env.objc.borrow_mut::<NSIndexPathHostObject>(this);
    host.indexes = vec;
    this
}

- (NSUInteger)indexAtPosition:(NSUInteger)position {
    let host = env.objc.borrow::<NSIndexPathHostObject>(this);
    if (position as usize) < host.indexes.len() {
        host.indexes[position as usize]
    } else {
        NSNotFound as NSUInteger
    }
}

- (NSUInteger)length {
    env.objc.borrow::<NSIndexPathHostObject>(this).indexes.len() as NSUInteger
}

- (NSInteger)section {
    let host = env.objc.borrow::<NSIndexPathHostObject>(this);
    if !host.indexes.is_empty() {
        host.indexes[0] as NSInteger
    } else {
        0
    }
}

- (NSInteger)row {
    let host = env.objc.borrow::<NSIndexPathHostObject>(this);
    if host.indexes.len() >= 2 {
        host.indexes[1] as NSInteger
    } else {
        0
    }
}

- (NSComparisonResult)compare:(id)other {
    let host_this = env.objc.borrow::<NSIndexPathHostObject>(this);
    let host_other = env.objc.borrow::<NSIndexPathHostObject>(other);
    match host_this.indexes.cmp(&host_other.indexes) {
        std::cmp::Ordering::Less => NSOrderedAscending,
        std::cmp::Ordering::Equal => NSOrderedSame,
        std::cmp::Ordering::Greater => NSOrderedDescending,
    }
}

- (id)copyWithZone:(NSZonePtr)_zone {
    retain(env, this)
}

- (bool)isEqual:(id)other {
    if this == other {
        return true;
    }
    let class: Class = msg_class![env; NSIndexPath class];
    if !msg![env; other isKindOfClass:class] {
        return false;
    }
    let host_this = env.objc.borrow::<NSIndexPathHostObject>(this);
    let host_other = env.objc.borrow::<NSIndexPathHostObject>(other);
    host_this.indexes == host_other.indexes
}

- (NSUInteger)hash {
    let host = env.objc.borrow::<NSIndexPathHostObject>(this);
    let mut hash: NSUInteger = host.indexes.len() as NSUInteger;
    for &idx in &host.indexes {
        hash = hash.wrapping_mul(31).wrapping_add(idx);
    }
    hash
}

@end

};
