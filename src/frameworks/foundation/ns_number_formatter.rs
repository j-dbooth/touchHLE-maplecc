/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSNumberFormatter`.

use super::ns_string::{from_rust_string, to_rust_string};
use super::NSUInteger;
use crate::objc::{
    autorelease, id, msg, nil, objc_classes, retain, ClassExports, HostObject, NSZonePtr,
};

struct NSNumberFormatterHostObject {
    number_style: NSUInteger,
}
impl HostObject for NSNumberFormatterHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSNumberFormatter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSNumberFormatterHostObject {
        number_style: 0,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)localizedStringFromNumber:(id)number numberStyle:(NSUInteger)style {
    if number == nil {
        return nil;
    }
    let str: id = msg![env; number stringValue];
    let s = to_rust_string(env, str);
    if style == 1 || style == 2 {
        if let Ok(val) = s.parse::<i64>() {
            let mut formatted = String::new();
            let val_str = val.abs().to_string();
            let len = val_str.len();
            for (i, c) in val_str.chars().enumerate() {
                if i > 0 && (len - i) % 3 == 0 {
                    formatted.push(',');
                }
                formatted.push(c);
            }
            if val < 0 {
                formatted.insert(0, '-');
            }
            return from_rust_string(env, formatted);
        }
    }
    retain(env, str);
    autorelease(env, str)
}

- (id)stringFromNumber:(id)number {
    if number == nil {
        return nil;
    }
    let style = env.objc.borrow::<NSNumberFormatterHostObject>(this).number_style;
    if style != 0 {
        let class: crate::objc::Class = msg![env; this class];
        return msg![env; class localizedStringFromNumber:number numberStyle:style];
    }
    let str: id = msg![env; number stringValue];
    retain(env, str);
    autorelease(env, str)
}

- (NSUInteger)numberStyle {
    env.objc.borrow::<NSNumberFormatterHostObject>(this).number_style
}

- (())setNumberStyle:(NSUInteger)style {
    env.objc.borrow_mut::<NSNumberFormatterHostObject>(this).number_style = style;
}

- (())setLocale:(id)_locale {
    // no-op
}

- (())setPositiveFormat:(id)_format {
    // no-op
}

- (())setNegativeFormat:(id)_format {
    // no-op
}

- (())setFormat:(id)_format {
    // no-op
}

- (())setGeneratesDecimalNumbers:(bool)_generates {
    // no-op
}

@end

};
