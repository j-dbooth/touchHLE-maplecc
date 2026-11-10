/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSCoder`.

use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::mem::{ConstPtr, MutPtr};
use crate::objc::{id, nil, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSCoder: NSObject
// This is an abstract class
- (bool)allowsKeyedCoding {
    false
}
- (bool)decodeBoolForKey:(id)_key {
    false
}
- (f32)decodeFloatForKey:(id)_key {
    0.0
}
- (f64)decodeDoubleForKey:(id)_key {
    0.0
}
- (NSInteger)decodeIntegerForKey:(id)_key {
    0
}
- (i32)decodeIntForKey:(id)_key {
    0
}
- (i32)decodeInt32ForKey:(id)_key {
    0
}
- (i64)decodeInt64ForKey:(id)_key {
    0
}
- (id)decodeObjectForKey:(id)_key {
    nil
}
- (ConstPtr<u8>)decodeBytesForKey:(id)_key returnedLength:(MutPtr<NSUInteger>)length {
    if !length.is_null() {
        env.mem.write(length, 0);
    }
    ConstPtr::null()
}
- (CGPoint)decodeCGPointForKey:(id)_key {
    CGPoint::default()
}
- (CGSize)decodeCGSizeForKey:(id)_key {
    CGSize::default()
}
- (CGRect)decodeCGRectForKey:(id)_key {
    CGRect::default()
}
- (bool)containsValueForKey:(id)_key {
    false
}
- (())encodeObject:(id)_object forKey:(id)_key {}
- (())encodeBool:(bool)_val forKey:(id)_key {}
- (())encodeInt:(i32)_val forKey:(id)_key {}
- (())encodeInt32:(i32)_val forKey:(id)_key {}
- (())encodeInt64:(i64)_val forKey:(id)_key {}
- (())encodeInteger:(NSInteger)_val forKey:(id)_key {}
- (())encodeFloat:(f32)_val forKey:(id)_key {}
- (())encodeDouble:(f64)_val forKey:(id)_key {}
- (())encodeCGPoint:(CGPoint)_point forKey:(id)_key {}
- (())encodeCGSize:(CGSize)_size forKey:(id)_key {}
- (())encodeCGRect:(CGRect)_rect forKey:(id)_key {}
@end

};

