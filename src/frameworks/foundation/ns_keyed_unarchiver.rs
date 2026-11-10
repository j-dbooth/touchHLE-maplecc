/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSKeyedUnarchiver` and deserialization of its object graph format.
//!
//! Resources:
//! - You can get a good intuitive grasp of how the format works just by staring
//!   at a pretty-print of a simple nib file from something that can parse
//!   plists, e.g. `plutil -p` or `println!("{:#?}", plist::Value::...);`.
//! - Apple's [Archives and Serializations Programming Guide](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Archiving/Articles/archives.html)

use super::ns_string::{from_rust_string, get_static_str, to_rust_string, NSUTF8StringEncoding};
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::frameworks::uikit::ui_geometry::{
    CGPointFromString, CGRectFromString, CGSizeFromString,
};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, ObjC,
};
use crate::Environment;
use plist::{Dictionary, Uid, Value};
use std::io::Cursor;

pub const NSKeyedArchiveRootObjectKey: &str = "root";

pub const CONSTANTS: ConstantExports = &[(
    "_NSKeyedArchiveRootObjectKey",
    HostConstant::NSString(NSKeyedArchiveRootObjectKey),
)];

struct NSKeyedUnarchiverHostObject {
    plist: Dictionary,
    current_key: Option<Uid>,
    /// linear map of Uid => id
    already_unarchived: Vec<Option<id>>,
    /// Something responding to NSKeyedUnarchiverDelegate
    delegate: id,
    /// Stores the buffers decoded by `decodeBytesForKey:returnedLength:`
    /// Instead of reusing the same buffer, we allocate different ones that get
    /// freed on dealloc. A similar behavior has been observed in real iOS.
    temporary_buffers: Vec<MutVoidPtr>,
}
impl HostObject for NSKeyedUnarchiverHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSKeyedUnarchiver: NSCoder

+ (id)allocWithZone:(NSZonePtr)_zone { // struct _NSZone*
    let unarchiver = Box::new(NSKeyedUnarchiverHostObject {
        plist: Dictionary::new(),
        current_key: None,
        already_unarchived: Vec::new(),
        delegate: nil,
        temporary_buffers: Vec::new(),
    });
    env.objc.alloc_object(this, unarchiver, &mut env.mem)
}

+ (id)unarchiveObjectWithFile:(id)path { // NSString *
    let data: id = msg_class![env; NSData dataWithContentsOfFile:path];
    if data == nil {
        return nil;
    }
    msg![env; this unarchiveObjectWithData:data]
}

+ (id)unarchiveObjectWithData:(id)data { // NSData *
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initForReadingWithData:data];
    let root_key = get_static_str(env, NSKeyedArchiveRootObjectKey);
    let result: id = msg![env; new decodeObjectForKey:root_key];
    autorelease(env, result)
}

- (id)init {
    this
}

- (id)initForReadingWithData:(id)data { // NSData *
    if data == nil {
        return nil;
    }

    let length: NSUInteger = msg![env; data length];
    if length == 0 {
        return nil;
    }
    let bytes: ConstVoidPtr = msg![env; data bytes];
    let slice = env.mem.bytes_at(bytes.cast(), length);

    let host_obj = env.objc.borrow_mut::<NSKeyedUnarchiverHostObject>(this);
    assert!(host_obj.already_unarchived.is_empty());
    assert!(host_obj.current_key.is_none());
    assert!(host_obj.plist.is_empty());

    let Ok(plist) = Value::from_reader(Cursor::new(slice)) else {
        return nil;
    };
    let Some(plist) = plist.into_dictionary() else {
        return nil;
    };
    if plist.get("$archiver").and_then(|v| v.as_string()) != Some("NSKeyedArchiver") {
        return nil;
    }
    let Some(objects) = plist.get("$objects").and_then(|v| v.as_array()) else {
        return nil;
    };
    let key_count = objects.len();

    host_obj.already_unarchived = vec![None; key_count];
    host_obj.plist = plist;

    this
}

- (())finishDecoding {
}

- (bool)allowsKeyedCoding {
    true
}

- (())dealloc {
    let host_obj = borrow_host_obj(env, this);
    let already_unarchived = std::mem::take(&mut host_obj.already_unarchived);
    let temporary_buffers = std::mem::take(&mut host_obj.temporary_buffers);

    for &object in already_unarchived.iter().flatten() {
        if object != nil && env.objc.is_object_alive(object) && ObjC::read_isa(object, &env.mem) != nil {
            release(env, object);
        }
    }

    for &buffer in temporary_buffers.iter() {
        env.mem.free(buffer);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: implement calls to delegate methods
// weak/non-retaining
- (())setDelegate:(id)delegate { // id<NSKeyedUnarchiverDelegate>
    let host_object = env.objc.borrow_mut::<NSKeyedUnarchiverHostObject>(this);
    host_object.delegate = delegate;
}
- (id)delegate {
    env.objc.borrow::<NSKeyedUnarchiverHostObject>(this).delegate
}

// These methods drive most of the decoding. They get called in two cases:
// - By the code that initiates the unarchival, e.g. UINib, to retrieve
//   top-level objects.
// - By the object currently being unarchived, i.e. something that had
//   `initWithCoder:` called on it, to retrieve objects from its scope.
// They are all from the NSCoder abstract class and they return default values
// if the key is unknown.

- (bool)decodeBoolForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(false, value_to_bool)
}

- (f64)decodeDoubleForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0.0, value_to_f64)
}

- (f32)decodeFloatForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0.0, value_to_f64) as f32
}

- (NSInteger)decodeIntegerForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0, value_to_i64) as NSInteger
}

- (i32)decodeIntForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0, value_to_i64) as i32
}

- (i32)decodeInt32ForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0, value_to_i64) as i32
}

- (i64)decodeInt64ForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(0, value_to_i64)
}

- (id)decodeObjectForKey:(id)key { // NSString*
    let Some(val) = get_value_to_decode_for_key(env, this, key).cloned() else {
        return nil;
    };
    let object = if let Some(next_uid) = val.as_uid() {
        unarchive_key(env, this, *next_uid)
    } else {
        match val {
            Value::String(s) if s == "$null" => nil,
            Value::String(s) => from_rust_string(env, s),
            Value::Boolean(b) => {
                let num: id = msg_class![env; NSNumber alloc];
                msg![env; num initWithBool:b]
            }
            Value::Integer(i) => {
                let num: id = msg_class![env; NSNumber alloc];
                if let Some(i64_val) = i.as_signed() {
                    msg![env; num initWithLongLong:i64_val]
                } else if let Some(u64_val) = i.as_unsigned() {
                    msg![env; num initWithUnsignedLongLong:u64_val]
                } else {
                    msg![env; num initWithInt:0]
                }
            }
            Value::Real(r) => {
                let num: id = msg_class![env; NSNumber alloc];
                msg![env; num initWithDouble:r]
            }
            Value::Data(d) => {
                let len: GuestUSize = d.len().try_into().unwrap();
                let guest_bytes: MutVoidPtr = env.mem.alloc(len);
                env.mem
                    .bytes_at_mut(guest_bytes.cast(), len)
                    .copy_from_slice(d.as_slice());
                let data_obj: id = msg_class![env; NSData alloc];
                msg![env; data_obj initWithBytesNoCopy:guest_bytes length:len freeWhenDone:true]
            }
            _ => nil,
        }
    };

    // on behalf of the caller
    retain(env, object);
    autorelease(env, object)
}

- (ConstPtr<u8>)decodeBytesForKey:(id)key returnedLength:(MutPtr<NSUInteger>)length {
    assert!(key != nil);
    let Some(data) = get_value_to_decode_for_key(env, this, key)
        .and_then(|value| value.as_data())
        .map(|data| data.to_vec()) else {
            if !length.is_null() {
                env.mem.write(length, 0);
            }
            return ConstPtr::null();
    };
    let len: GuestUSize = data.len().try_into().unwrap();
    let guest_bytes: MutVoidPtr = env.mem.alloc(len);
    env.objc.borrow_mut::<NSKeyedUnarchiverHostObject>(this)
        .temporary_buffers
        .push(guest_bytes);
    env.mem
        .bytes_at_mut(guest_bytes.cast(), len)
        .copy_from_slice(data.as_slice());
    if !length.is_null() {
        env.mem.write(length, len);
    }
    guest_bytes.cast().cast_const()
}

- (bool)containsValueForKey:(id)key { // NSString*
    assert!(key != nil);
    get_value_to_decode_for_key(env, this, key).is_some()
}

// TODO: add more decode methods

// These come from a category in UIKit's UIGeometry.h
- (CGPoint)decodeCGPointForKey:(id)key { // NSString*
    let string: id = msg![env; this decodeObjectForKey:key];
    CGPointFromString(env, string)
}
- (CGSize)decodeCGSizeForKey:(id)key { // NSString*
    let string: id = msg![env; this decodeObjectForKey:key];
    CGSizeFromString(env, string)
}
- (CGRect)decodeCGRectForKey:(id)key { // NSString*
    let string: id = msg![env; this decodeObjectForKey:key];
    CGRectFromString(env, string)
}

@end

};

fn value_to_bool(value: &Value) -> bool {
    match value {
        Value::Boolean(b) => *b,
        Value::Integer(i) => i.as_signed().or_else(|| i.as_unsigned().map(|u| u as i64)).unwrap_or(0) != 0,
        _ => false,
    }
}

fn value_to_i64(value: &Value) -> i64 {
    match value {
        Value::Integer(i) => i.as_signed().or_else(|| i.as_unsigned().map(|u| u as i64)).unwrap_or(0),
        Value::Boolean(b) => if *b { 1 } else { 0 },
        Value::Real(r) => *r as i64,
        _ => 0,
    }
}

fn value_to_f64(value: &Value) -> f64 {
    match value {
        Value::Real(r) => *r,
        Value::Integer(i) => i.as_signed().or_else(|| i.as_unsigned().map(|u| u as i64)).map(|v| v as f64).unwrap_or(0.0),
        Value::Boolean(b) => if *b { 1.0 } else { 0.0 },
        _ => 0.0,
    }
}

fn borrow_host_obj(env: &mut Environment, unarchiver: id) -> &mut NSKeyedUnarchiverHostObject {
    env.objc.borrow_mut(unarchiver)
}

fn get_value_to_decode_for_key(env: &mut Environment, unarchiver: id, key: id) -> Option<&Value> {
    let key = to_rust_string(env, key); // TODO: avoid copying string
    let host_obj = borrow_host_obj(env, unarchiver);
    let dict = match host_obj.current_key {
        Some(current_uid) => {
            let objects = host_obj.plist.get("$objects")?.as_array()?;
            objects.get(current_uid.get() as usize)?.as_dictionary()?
        }
        None => host_obj.plist.get("$top")?.as_dictionary()?,
    };
    dict.get(&key)
}

/// The core of the implementation: unarchive something by its uid.
///
/// This is recursive in practice: the `initWithCoder:` messages sent by this
/// function will be received by objects which will then send
/// `decodeXXXWithKey:` messages back to the unarchiver, which will then call
/// this function (and so on).
///
/// The object returned is retained only by the archiver. Remember to retain and
/// possibly autorelease it as appropriate.
fn unarchive_key(env: &mut Environment, unarchiver: id, key: Uid) -> id {
    if key.get() == 0 {
        return nil;
    }
    {
        let host_obj = borrow_host_obj(env, unarchiver);
        if let Some(existing) = host_obj.already_unarchived[key.get() as usize] {
            return existing;
        }
    }

    let item = borrow_host_obj(env, unarchiver).plist["$objects"].as_array().unwrap()[key.get() as usize].clone();

    let new_object = match item {
        Value::String(s) if s == "$null" => nil,
        Value::String(s) => from_rust_string(env, s),
        Value::Boolean(val) => {
            let number: id = msg_class![env; NSNumber alloc];
            msg![env; number initWithBool:val]
        }
        Value::Integer(int) => {
            let number: id = msg_class![env; NSNumber alloc];
            if let Some(int64) = int.as_signed() {
                msg![env; number initWithLongLong:int64]
            } else if let Some(uint64) = int.as_unsigned() {
                msg![env; number initWithUnsignedLongLong:uint64]
            } else {
                msg![env; number initWithInt:0]
            }
        }
        Value::Real(val) => {
            let number: id = msg_class![env; NSNumber alloc];
            msg![env; number initWithDouble:val]
        }
        Value::Data(data) => {
            let len: GuestUSize = data.len().try_into().unwrap();
            let guest_bytes: MutVoidPtr = env.mem.alloc(len);
            env.mem
                .bytes_at_mut(guest_bytes.cast(), len)
                .copy_from_slice(data.as_slice());
            let data_obj: id = msg_class![env; NSData alloc];
            msg![env; data_obj initWithBytesNoCopy:guest_bytes length:len freeWhenDone:true]
        }
        Value::Array(arr) => {
            let array: id = msg_class![env; NSMutableArray alloc];
            let array: id = msg![env; array init];
            borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] = Some(array);
            for val in arr {
                if let Some(uid) = val.as_uid() {
                    let obj = unarchive_key(env, unarchiver, *uid);
                    () = msg![env; array addObject:obj];
                }
            }
            array
        }
        Value::Dictionary(dict) => {
            if let Some(class_key) = dict.get("$class").and_then(|c| c.as_uid().copied()) {
                let class_item = borrow_host_obj(env, unarchiver).plist["$objects"].as_array().unwrap()[class_key.get() as usize].clone();
                let class_dict = class_item.as_dictionary().unwrap();
                let class_name = class_dict.get("$classname").and_then(|v| v.as_string()).unwrap_or("");
                let mut resolved_class = env.objc.find_class(class_name, &mut env.mem);
                if resolved_class.is_none() {
                    if let Some(classes) = class_dict.get("$classes").and_then(|v| v.as_array()) {
                        for c in classes {
                            if let Some(c_name) = c.as_string() {
                                if let Some(cls) = env.objc.find_class(c_name, &mut env.mem) {
                                    resolved_class = Some(cls);
                                    break;
                                }
                            }
                        }
                    }
                }
                let class = resolved_class.unwrap_or_else(|| env.objc.link_class(class_name, false, &mut env.mem));

                let old_current_key = borrow_host_obj(env, unarchiver).current_key;
                borrow_host_obj(env, unarchiver).current_key = Some(key);

                let allocated_object: id = msg![env; class alloc];
                borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] = Some(allocated_object);
                let new_object: id = msg![env; allocated_object initWithCoder:unarchiver];

                borrow_host_obj(env, unarchiver).current_key = old_current_key;

                new_object
            } else {
                let ns_dict: id = msg_class![env; NSMutableDictionary alloc];
                let ns_dict: id = msg![env; ns_dict init];
                borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] = Some(ns_dict);
                for (k, v) in dict {
                    if let Some(uid) = v.as_uid() {
                        let obj = unarchive_key(env, unarchiver, *uid);
                        let k_ns = from_rust_string(env, k);
                        () = msg![env; ns_dict setObject:obj forKey:k_ns];
                    }
                }
                ns_dict
            }
        }
        _ => unimplemented!("Unarchive: {:#?}", item),
    };

    borrow_host_obj(env, unarchiver).already_unarchived[key.get() as usize] = Some(new_object);
    new_object
}

/// Shortcut for use by `[_touchHLE_NSArray initWithCoder:]`.
///
/// The objects are to be considered retained by the `Vec`.
pub fn decode_current_array(env: &mut Environment, unarchiver: id) -> Vec<id> {
    let keys = keys_for_key(env, unarchiver, "NS.objects");

    keys.into_iter()
        .map(|key| {
            let new_object = unarchive_key(env, unarchiver, key);
            // object is retained by the Vec
            retain(env, new_object)
        })
        .collect()
}

/// Shortcut for use by `[_touchHLE_NSMutableDictionary initWithCoder:]`.
///
/// Similar to `decode_current_array`, but for dictionaries.
/// The keys and objects are not retained!
pub fn decode_current_dict(env: &mut Environment, unarchiver: id) -> Vec<(id, id)> {
    let keys = keys_for_key(env, unarchiver, "NS.keys");
    let vals = keys_for_key(env, unarchiver, "NS.objects");
    log_dbg!("decode_current_dict: keys {:?}, vals {:?}", keys, vals);

    let keys: Vec<id> = keys
        .into_iter()
        .map(|key| unarchive_key(env, unarchiver, key))
        .collect();
    let vals: Vec<id> = vals
        .into_iter()
        .map(|val| unarchive_key(env, unarchiver, val))
        .collect();

    keys.into_iter().zip(vals).collect()
}

/// Shortcut for use by `[NSDate initWithCoder:]`.
pub fn decode_current_date(env: &mut Environment, unarchiver: id) -> id {
    let key = get_static_str(env, "NS.time");
    let timestamp = get_value_to_decode_for_key(env, unarchiver, key)
        .map_or(0.0, value_to_f64);

    let date: id = msg_class![env; NSDate alloc];
    msg![env; date initWithTimeIntervalSinceReferenceDate:timestamp]
}

/// Shortcut for use by `[NSData initWithCoder:]`.
pub fn decode_current_data(env: &mut Environment, unarchiver: id, is_mutable: bool) -> id {
    let key = get_static_str(env, "NS.data");
    let Some(bytes) = get_value_to_decode_for_key(env, unarchiver, key)
        .and_then(|v| v.as_data())
        .map(|d| d.to_vec()) else {
        return if is_mutable {
            let data: id = msg_class![env; NSMutableData alloc];
            msg![env; data init]
        } else {
            let data: id = msg_class![env; NSData alloc];
            msg![env; data init]
        };
    };
    let len: GuestUSize = bytes.len().try_into().unwrap();
    let guest_bytes: MutVoidPtr = env.mem.alloc(len);
    env.mem
        .bytes_at_mut(guest_bytes.cast(), len)
        .copy_from_slice(bytes.as_slice());

    if is_mutable {
        let data: id = msg_class![env; NSMutableData alloc];
        msg![env; data initWithBytesNoCopy:guest_bytes length:len freeWhenDone:true]
    } else {
        let data: id = msg_class![env; NSData alloc];
        msg![env; data initWithBytesNoCopy:guest_bytes length:len freeWhenDone:true]
    }
}

/// Shortcut for use by `[NSString initWithCoder:]`.
/// TODO: mutability
pub fn decode_current_string(env: &mut Environment, unarchiver: id) -> id {
    let key_bytes = get_static_str(env, "NS.bytes");
    let val_bytes = get_value_to_decode_for_key(env, unarchiver, key_bytes).cloned();
    if let Some(val) = val_bytes {
        if let Some(bytes) = val.as_data() {
            let len: GuestUSize = bytes.len().try_into().unwrap();
            let guest_bytes: ConstPtr<u8> = env.mem.alloc(len).cast().cast_const();
            env.mem
                .bytes_at_mut(guest_bytes.cast_mut(), len)
                .copy_from_slice(bytes);

            let str: id = msg_class![env; NSString alloc];
            let res = msg![env; str initWithBytes:guest_bytes length:len encoding:NSUTF8StringEncoding];
            env.mem.free(guest_bytes.cast().cast_mut());
            return res;
        } else if let Some(s) = val.as_string() {
            return from_rust_string(env, s.to_string());
        }
    }
    let key_string = get_static_str(env, "NS.string");
    let val_string = get_value_to_decode_for_key(env, unarchiver, key_string).cloned();
    if let Some(val) = val_string {
        if let Some(s) = val.as_string() {
            return from_rust_string(env, s.to_string());
        }
    }
    from_rust_string(env, "".to_string())
}

/// Shortcut for use by `[NSNumber initWithCoder:]`.
pub fn decode_current_number(env: &mut Environment, unarchiver: id) -> id {
    let num: id = msg_class![env; NSNumber alloc];
    let int_key = get_static_str(env, "NS.intval");
    let dbl_key = get_static_str(env, "NS.dblval");
    let real_key = get_static_str(env, "NS.realval");
    let bool_key = get_static_str(env, "NS.boolval");
    if msg![env; unarchiver containsValueForKey:int_key] {
        let longlong: i64 = msg![env; unarchiver decodeInt64ForKey:int_key];
        msg![env; num initWithLongLong:longlong]
    } else if msg![env; unarchiver containsValueForKey:dbl_key] {
        let double: f64 = msg![env; unarchiver decodeDoubleForKey:dbl_key];
        msg![env; num initWithDouble:double]
    } else if msg![env; unarchiver containsValueForKey:real_key] {
        let double: f64 = msg![env; unarchiver decodeDoubleForKey:real_key];
        msg![env; num initWithDouble:double]
    } else if msg![env; unarchiver containsValueForKey:bool_key] {
        let boolean: bool = msg![env; unarchiver decodeBoolForKey:bool_key];
        msg![env; num initWithBool:boolean]
    } else {
        msg![env; num initWithInt:0]
    }
}

fn keys_for_key(env: &mut Environment, unarchiver: id, key: &str) -> Vec<Uid> {
    let host_obj = borrow_host_obj(env, unarchiver);
    let Some(objects) = host_obj.plist.get("$objects").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let Some(current_uid) = host_obj.current_key else {
        return Vec::new();
    };
    let Some(item) = objects.get(current_uid.get() as usize).and_then(|v| v.as_dictionary()) else {
        return Vec::new();
    };
    let Some(keys) = item.get(key).and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    keys.iter()
        .filter_map(|value| value.as_uid().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_conversions() {
        assert_eq!(value_to_bool(&Value::Boolean(true)), true);
        assert_eq!(value_to_bool(&Value::Boolean(false)), false);
        assert_eq!(value_to_bool(&Value::Integer(1.into())), true);
        assert_eq!(value_to_bool(&Value::Integer(0.into())), false);
        assert_eq!(value_to_bool(&Value::String("hello".into())), false);

        assert_eq!(value_to_i64(&Value::Integer(42.into())), 42);
        assert_eq!(value_to_i64(&Value::Boolean(true)), 1);
        assert_eq!(value_to_i64(&Value::Boolean(false)), 0);
        assert_eq!(value_to_i64(&Value::Real(3.14)), 3);
        assert_eq!(value_to_i64(&Value::String("foo".into())), 0);

        assert_eq!(value_to_f64(&Value::Real(2.718)), 2.718);
        assert_eq!(value_to_f64(&Value::Integer(10.into())), 10.0);
        assert_eq!(value_to_f64(&Value::Boolean(true)), 1.0);
        assert_eq!(value_to_f64(&Value::Boolean(false)), 0.0);
        assert_eq!(value_to_f64(&Value::String("bar".into())), 0.0);
    }

    #[test]
    fn test_edge_value_conversions() {
        assert_eq!(value_to_bool(&Value::Integer((-1).into())), true);
        assert_eq!(value_to_bool(&Value::Integer(0.into())), false);
        assert_eq!(value_to_i64(&Value::Integer((-42).into())), -42);
        assert_eq!(value_to_i64(&Value::Integer(0.into())), 0);
        assert_eq!(value_to_f64(&Value::Integer((-10).into())), -10.0);
        assert_eq!(value_to_f64(&Value::Real(0.0)), 0.0);
        assert_eq!(value_to_f64(&Value::Real(-2.5)), -2.5);
    }
}
