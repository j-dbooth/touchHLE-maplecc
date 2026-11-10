/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! UIKit NIB Archives decoder. This is _not_ a part of public API!
//!
//! Resources:
//! - [UIKit NIB Archives](https://www.mothersruin.com/software/Archaeology/reverse/uinib.html)
//! - [NibArchive File Format](https://github.com/matsmattsson/nibsqueeze/blob/master/NibArchive.md)

use crate::environment::Environment;
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use nibarchive::{NIBArchive, Value, ValueVariant};

struct NIBArchiveDecoderHostObject {
    archive: NIBArchive,
    current_object_idx: Option<u32>,
    /// linear map of idx => id
    already_unarchived: Vec<Option<id>>,
    delegate: id,
    temporary_buffers: Vec<MutVoidPtr>,
}
impl HostObject for NIBArchiveDecoderHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation _touchHLE_NIBArchiveDecoder: NSCoder

+ (id)allocWithZone:(NSZonePtr)_zone { // struct _NSZone*
    let unarchiver = Box::new(NIBArchiveDecoderHostObject {
        archive: NIBArchive::new_unchecked(
            Default::default(), Default::default(), Default::default(), Default::default()
        ),
        current_object_idx: None,
        already_unarchived: Vec::new(),
        delegate: nil,
        temporary_buffers: Vec::new(),
    });
    env.objc.alloc_object(this, unarchiver, &mut env.mem)
}

- (id)_touchHLE_initForReadingWithData:(id)data {
    if data == nil {
        return nil;
    }

    let length: NSUInteger = msg![env; data length];
    let bytes: ConstVoidPtr = msg![env; data bytes];

    let archive: NIBArchive = NIBArchive::from_bytes(env.mem.bytes_at(bytes.cast(), length)).unwrap();

    // for (i, object) in archive.objects().iter().enumerate() {
    //     let class_name = object.class_name(archive.class_names()).name();
    //     println!("[{i}] Object of a class '{class_name}':");
    //
    //     let values: &[nibarchive::Value] = object.values(archive.values());
    //     for (j, value) in values.iter().enumerate() {
    //         let key = value.key(archive.keys());
    //         let inner_value = value.value();
    //         println!("-- [{j}] {key}: {inner_value:?}");
    //     }
    // }

    let host_obj = env.objc.borrow_mut::<NIBArchiveDecoderHostObject>(this);
    assert!(host_obj.already_unarchived.is_empty());
    assert!(host_obj.current_object_idx.is_none());

    let objects_count = archive.objects().len();

    host_obj.already_unarchived = vec![None; objects_count];
    host_obj.archive = archive;

    // we start from the 'top'
    host_obj.current_object_idx = Some(0);

    this
}

- (())dealloc {
    let host_obj: &mut NIBArchiveDecoderHostObject = env.objc.borrow_mut(this);
    let already_unarchived = std::mem::take(&mut host_obj.already_unarchived);
    let temporary_buffers = std::mem::take(&mut host_obj.temporary_buffers);

    for &object in already_unarchived.iter().flatten() {
        release(env, object);
    }
    for &buffer in temporary_buffers.iter() {
        env.mem.free(buffer);
    }

    env.objc.dealloc_object(this, &mut env.mem)
}

// TODO: implement calls to delegate methods
// weak/non-retaining
- (())setDelegate:(id)delegate { // id<?>
    let host_object = env.objc.borrow_mut::<NIBArchiveDecoderHostObject>(this);
    host_object.delegate = delegate;
}
- (id)delegate {
    env.objc.borrow::<NIBArchiveDecoderHostObject>(this).delegate
}

// These methods drive most of the decoding. They get called in two cases:
// - By the code that initiates the unarchival, e.g. UINib, to retrieve
//   top-level objects.
// - By the object currently being unarchived, i.e. something that had
//   `initWithCoder:` called on it, to retrieve objects from its scope.
// They are all from the NSCoder abstract class and they return default values
// if the key is unknown.

- (bool)decodeBoolForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).is_some_and(|value| {
        match value.value() {
            ValueVariant::Bool(b) => *b,
            ValueVariant::Int8(i) => *i != 0,
            ValueVariant::Int16(i) => *i != 0,
            ValueVariant::Int32(i) => *i != 0,
            ValueVariant::Int64(i) => *i != 0,
            _ => false,
        }
    })
}

- (f32)decodeFloatForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(
        0.0,
        |value| {
            match value.value() {
                ValueVariant::Float(f) => *f,
                ValueVariant::Double(d) => *d as f32,
                ValueVariant::Int8(i) => *i as f32,
                ValueVariant::Int16(i) => *i as f32,
                ValueVariant::Int32(i) => *i as f32,
                ValueVariant::Int64(i) => *i as f32,
                ValueVariant::Bool(b) => if *b { 1.0 } else { 0.0 },
                _ => 0.0,
            }
        }
    )
}

- (f64)decodeDoubleForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(
        0.0,
        |value| {
            match value.value() {
                ValueVariant::Double(d) => *d,
                ValueVariant::Float(f) => *f as f64,
                ValueVariant::Int8(i) => *i as f64,
                ValueVariant::Int16(i) => *i as f64,
                ValueVariant::Int32(i) => *i as f64,
                ValueVariant::Int64(i) => *i as f64,
                ValueVariant::Bool(b) => if *b { 1.0 } else { 0.0 },
                _ => 0.0,
            }
        }
    )
}

- (NSInteger)decodeIntegerForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(
        0,
        |value| {
            match value.value() {
                ValueVariant::Int8(i) => (*i).into(),
                ValueVariant::Int16(i) => (*i).into(),
                ValueVariant::Int32(i) => *i,
                ValueVariant::Int64(i) => *i as NSInteger,
                ValueVariant::Float(f) => *f as NSInteger,
                ValueVariant::Double(d) => *d as NSInteger,
                ValueVariant::Bool(b) => if *b { 1 } else { 0 },
                _ => 0,
            }
        }
    )
}
- (i32)decodeIntForKey:(id)key { // NSString *
    msg![env; this decodeIntegerForKey:key]
}
- (i32)decodeInt32ForKey:(id)key { // NSString *
    msg![env; this decodeIntegerForKey:key]
}
- (i64)decodeInt64ForKey:(id)key { // NSString *
    get_value_to_decode_for_key(env, this, key).map_or(
        0,
        |value| {
            match value.value() {
                ValueVariant::Int8(i) => (*i).into(),
                ValueVariant::Int16(i) => (*i).into(),
                ValueVariant::Int32(i) => (*i).into(),
                ValueVariant::Int64(i) => *i,
                ValueVariant::Float(f) => *f as i64,
                ValueVariant::Double(d) => *d as i64,
                ValueVariant::Bool(b) => if *b { 1 } else { 0 },
                _ => 0,
            }
        }
    )
}

- (id)decodeObjectForKey:(id)key { // NSString*
    let Some(val) = get_value_to_decode_for_key(env, this, key) else {
        return nil;
    };
    let &ValueVariant::ObjectRef(idx) = val.value() else {
        return nil;
    };

    let object = unarchive_obj(env, this, idx);

    // on behalf of the caller
    retain(env, object);
    autorelease(env, object)
}

- (ConstPtr<u8>)decodeBytesForKey:(id)key returnedLength:(MutPtr<NSUInteger>)length {
    assert!(key != nil);
    let data = get_value_to_decode_for_key(env, this, key).and_then(|val| {
        if let ValueVariant::Data(data) = val.value() {
            Some(data.clone())
        } else {
            None
        }
    });
    let Some(data) = data else {
        if !length.is_null() {
            env.mem.write(length, 0);
        }
        return ConstPtr::null();
    };
    let len: GuestUSize = data.len().try_into().unwrap();
    let guest_bytes: MutVoidPtr = env.mem.alloc(len);
    env.objc
        .borrow_mut::<NIBArchiveDecoderHostObject>(this)
        .temporary_buffers
        .push(guest_bytes);
    env.mem
        .bytes_at_mut(guest_bytes.cast(), len)
        .copy_from_slice(data.as_slice());
    if !length.is_null() {
        env.mem.write(length, len as NSUInteger);
    }
    guest_bytes.cast().cast_const()
}

- (bool)containsValueForKey:(id)key { // NSString*
    assert!(key != nil);
    get_value_to_decode_for_key(env, this, key).is_some()
}

// These come from a category in UIKit's UIGeometry.h
- (CGPoint)decodeCGPointForKey:(id)key { // NSString*
    let Some(val) = get_value_to_decode_for_key(env, this, key) else {
        return CGPoint::default();
    };
    let ValueVariant::Data(data) = val.value() else {
        return CGPoint::default();
    };
    if data.len() < 9 || data[0] != 6 {
        return CGPoint::default();
    }
    let x = f32::from_le_bytes(data[1..5].try_into().unwrap());
    let y = f32::from_le_bytes(data[5..9].try_into().unwrap());
    log_dbg!("decoded CGPoint {} {}", x, y);
    CGPoint { x, y }
}

- (CGSize)decodeCGSizeForKey:(id)key { // NSString*
    let Some(val) = get_value_to_decode_for_key(env, this, key) else {
        return CGSize::default();
    };
    let ValueVariant::Data(data) = val.value() else {
        return CGSize::default();
    };
    if data.len() < 9 || data[0] != 6 {
        return CGSize::default();
    }
    let width = f32::from_le_bytes(data[1..5].try_into().unwrap());
    let height = f32::from_le_bytes(data[5..9].try_into().unwrap());
    log_dbg!("decoded CGSize {} {}", width, height);
    CGSize { width, height }
}

- (CGRect)decodeCGRectForKey:(id)key { // NSString*
    let Some(val) = get_value_to_decode_for_key(env, this, key) else {
        return CGRect::default();
    };
    let ValueVariant::Data(data) = val.value() else {
        return CGRect::default();
    };
    if data.len() < 17 || data[0] != 6 {
        return CGRect::default();
    }
    let x = f32::from_le_bytes(data[1..5].try_into().unwrap());
    let y = f32::from_le_bytes(data[5..9].try_into().unwrap());
    let width = f32::from_le_bytes(data[9..13].try_into().unwrap());
    let height = f32::from_le_bytes(data[13..17].try_into().unwrap());
    log_dbg!("decoded CGRect {} {} {} {}", x, y, width, height);
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

@end

};

fn borrow_host_obj(env: &mut Environment, unarchiver: id) -> &mut NIBArchiveDecoderHostObject {
    env.objc.borrow_mut(unarchiver)
}

fn get_value_to_decode_for_key(env: &mut Environment, unarchiver: id, key: id) -> Option<&Value> {
    let key = to_rust_string(env, key); // TODO: avoid copying string
    let host_obj = borrow_host_obj(env, unarchiver);
    let current_idx = host_obj.current_object_idx?;

    let obj = host_obj
        .archive
        .objects()
        .get(current_idx as usize)?;
    obj.values(host_obj.archive.values())
        .iter()
        .find(|&val| val.key(host_obj.archive.keys()) == &key)
        .map(|v| v as _)
}

fn unarchive_obj(env: &mut Environment, unarchiver: id, idx: u32) -> id {
    let host_obj = borrow_host_obj(env, unarchiver);
    if let Some(existing) = host_obj.already_unarchived[idx as usize] {
        return existing;
    }

    let object = host_obj.archive.objects().get(idx as usize).unwrap();
    let class_name = object.class_name(host_obj.archive.class_names()).name();

    log_dbg!(
        "Unarchiving object of a class '{}' at index {}",
        class_name,
        idx
    );

    let class = {
        // get_known_class needs &mut ObjC, so we can't call it
        // while holding a reference to the class name, since it
        // is ultimately owned by ObjC via the host object
        let class_name = class_name.to_string();
        if let Some(cls) = env.objc.find_class(&class_name, &mut env.mem) {
            cls
        } else {
            log!("Warning: NIB class '{}' not recognized, falling back to NSObject", class_name);
            env.objc.get_known_class("NSObject", &mut env.mem)
        }
    };

    let host_obj = borrow_host_obj(env, unarchiver); // reborrow
    let old_current_idx = host_obj.current_object_idx;
    host_obj.current_object_idx = Some(idx);

    let new_object: id = msg![env; class alloc];
    let init_with_coder_sel = env.objc.register_host_selector("initWithCoder:".to_string(), &mut env.mem);
    let new_object: id = if env.objc.object_has_method(&env.mem, new_object, init_with_coder_sel) {
        msg![env; new_object initWithCoder:unarchiver]
    } else {
        msg![env; new_object init]
    };

    let host_obj = borrow_host_obj(env, unarchiver); // reborrow
    host_obj.current_object_idx = old_current_idx;

    let host_obj = borrow_host_obj(env, unarchiver); // reborrow
    host_obj.already_unarchived[idx as usize] = Some(new_object);
    new_object
}

pub fn decode_current_array(env: &mut Environment, unarchiver: id) -> Vec<id> {
    let host_obj = borrow_host_obj(env, unarchiver);
    let current_idx = host_obj.current_object_idx.unwrap();

    let mut indicies = vec![];

    let object = host_obj
        .archive
        .objects()
        .get(current_idx as usize)
        .unwrap();
    let values: &[Value] = object.values(host_obj.archive.values());
    for (i, value) in values.iter().enumerate() {
        let key = value.key(host_obj.archive.keys());
        let inner_value = value.value();
        if i == 0 {
            assert_eq!("NSInlinedValue", key);
            let &ValueVariant::Bool(b) = inner_value else {
                unreachable!()
            };
            assert!(b); //  Can this ever be false?
            continue;
        }
        assert_eq!("UINibEncoderEmptyKey", key);

        let &ValueVariant::ObjectRef(next_idx) = inner_value else {
            unreachable!()
        };
        indicies.push(next_idx);
    }

    let mut array: Vec<id> = vec![];
    for next_idx in indicies {
        let host_obj = borrow_host_obj(env, unarchiver); // reborrow
        let old_current_idx = host_obj.current_object_idx;
        host_obj.current_object_idx = Some(next_idx);

        let next_obj = unarchive_obj(env, unarchiver, next_idx);
        retain(env, next_obj);
        array.push(next_obj);

        let host_obj = borrow_host_obj(env, unarchiver); // reborrow
        host_obj.current_object_idx = old_current_idx;
    }
    array
}

pub fn decode_current_dict(env: &mut Environment, unarchiver: id) -> Vec<(id, id)> {
    // Trick: dict is an array of key-value-key-value sequence
    let array = decode_current_array(env, unarchiver);
    // Note: we need to release as `dict_from_keys_and_objects`
    // would retain them later
    for &obj in &array {
        release(env, obj);
    }
    array
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunks| (chunks[0], chunks[1]))
        .collect()
}

pub fn decode_current_string(env: &mut Environment, unarchiver: id) -> id {
    let host_obj = borrow_host_obj(env, unarchiver);
    let current_idx = host_obj.current_object_idx.unwrap();

    let object = host_obj
        .archive
        .objects()
        .get(current_idx as usize)
        .unwrap();
    let values: &[Value] = object.values(host_obj.archive.values());
    assert_eq!(1, values.len());

    let value = &values[0];
    let key = value.key(host_obj.archive.keys());
    assert_eq!("NS.bytes", key);

    let inner_value = value.value();
    let ValueVariant::Data(inner_data) = inner_value else {
        unreachable!()
    };

    let str = String::from_utf8(inner_data.clone()).unwrap();
    log_dbg!("decode_current_string {str}");
    from_rust_string(env, str)
}

pub fn decode_current_number(env: &mut Environment, unarchiver: id) -> id {
    let key = {
        let host_obj = borrow_host_obj(env, unarchiver);
        let Some(current_idx) = host_obj.current_object_idx else {
            let num = msg_class![env; NSNumber alloc];
            return msg![env; num initWithInt:0];
        };

        let Some(object) = host_obj
            .archive
            .objects()
            .get(current_idx as usize) else {
            let num = msg_class![env; NSNumber alloc];
            return msg![env; num initWithInt:0];
        };
        let values: &[Value] = object.values(host_obj.archive.values());
        // Find if any value key matches NS.intval, NS.realval, NS.dblval, NS.boolval
        let matching_key = values.iter().find_map(|v| {
            let k = v.key(host_obj.archive.keys()).as_str();
            match k {
                "NS.intval" | "NS.realval" | "NS.dblval" | "NS.boolval" => Some(k.to_string()),
                _ => None,
            }
        });
        matching_key.unwrap_or_else(|| {
            if values.len() == 1 {
                values[0].key(host_obj.archive.keys()).to_string()
            } else {
                String::new()
            }
        })
    };

    match key.as_str() {
        "NS.intval" => {
            let ns_key: id = get_static_str(env, "NS.intval");
            let int: i64 = msg![env; unarchiver decodeInt64ForKey:ns_key];
            let num = msg_class![env; NSNumber alloc];
            msg![env; num initWithLongLong:int]
        }
        "NS.realval" | "NS.dblval" => {
            let ns_key: id = get_static_str(env, if key.as_str() == "NS.realval" { "NS.realval" } else { "NS.dblval" });
            let val: f64 = msg![env; unarchiver decodeDoubleForKey:ns_key];
            let num = msg_class![env; NSNumber alloc];
            msg![env; num initWithDouble:val]
        }
        "NS.boolval" => {
            let ns_key: id = get_static_str(env, "NS.boolval");
            let val: bool = msg![env; unarchiver decodeBoolForKey:ns_key];
            let num = msg_class![env; NSNumber alloc];
            msg![env; num initWithBool:val]
        }
        _ => {
            let num = msg_class![env; NSNumber alloc];
            msg![env; num initWithInt:0]
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn test_audit_all_nib_files() {
        let nib_names = [
            "ActiveSkillsPane.nib",
            "BuyConfirmPanel.nib",
            "DropItemPanel.nib",
            "EarnedSkillPanel.nib",
            "EnchantPanel.nib",
            "EquipmentCell.nib",
            "InventoryPanel.nib",
            "ItemCell.nib",
            "ItemGeneratorPanel.nib",
            "MainWindow.nib",
            "ManualPanel.nib",
            "NoticePanel.nib",
            "PassiveSkillsPane.nib",
            "SellConfirmPanel.nib",
            "ShopItemCell.nib",
            "SkillCell.nib",
            "StatusPane.nib",
            "StatusPanel.nib",
            "StorePanel.nib",
            "TestBattlePanel.nib",
        ];

        fn resolve_str(archive: &NIBArchive, obj_idx: usize) -> Option<String> {
            let obj = archive.objects().get(obj_idx)?;
            let values = obj.values(archive.values());
            for v in values {
                if v.key(archive.keys()) == "NS.bytes" {
                    if let ValueVariant::Data(bytes) = v.value() {
                        return String::from_utf8(bytes.clone()).ok();
                    }
                }
            }
            None
        }

        let mut all_classes = BTreeSet::new();
        let mut all_keys = BTreeSet::new();
        let mut class_to_keys: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut all_swapper_classes = BTreeSet::new();
        let mut all_outlet_labels = BTreeSet::new();
        let mut all_event_selectors = BTreeSet::new();

        for nib_name in &nib_names {
            let nib_path = format!("../touchHLE/touchHLE_apps/MapleCC.app/{nib_name}");
            let bytes = std::fs::read(&nib_path).unwrap_or_else(|_| {
                std::fs::read(format!("../extracted/Payload/MapleCC.app/{nib_name}")).expect("nib file exists")
            });

            assert!(bytes.len() >= 10);
            assert_eq!(&bytes[0..10], b"NIBArchive", "Nib {nib_name} is NIBArchive");

            let archive = NIBArchive::from_bytes(&bytes).expect("valid NIBArchive");
            for object in archive.objects().iter() {
                let class_name = object.class_name(archive.class_names()).name().to_string();
                all_classes.insert(class_name.clone());
                let entry = class_to_keys.entry(class_name.clone()).or_default();
                let values = object.values(archive.values());
                for val in values {
                    let key = val.key(archive.keys()).to_string();
                    all_keys.insert(key.clone());
                    entry.insert(key.clone());
                }
                let class_name = object.class_name(archive.class_names()).name().to_string();
                let values = object.values(archive.values());

                if class_name == "UIClassSwapper" {
                    for v in values {
                        if v.key(archive.keys()) == "UIClassName" {
                            if let ValueVariant::ObjectRef(s_idx) = v.value() {
                                if let Some(s) = resolve_str(&archive, *s_idx as usize) {
                                    all_swapper_classes.insert(s);
                                }
                            }
                        }
                    }
                } else if class_name == "UIRuntimeOutletConnection" {
                    for v in values {
                        if v.key(archive.keys()) == "UILabel" {
                            if let ValueVariant::ObjectRef(s_idx) = v.value() {
                                if let Some(s) = resolve_str(&archive, *s_idx as usize) {
                                    all_outlet_labels.insert(s);
                                }
                            }
                        }
                    }
                } else if class_name == "UIRuntimeEventConnection" {
                    for v in values {
                        if v.key(archive.keys()) == "UILabel" {
                            if let ValueVariant::ObjectRef(s_idx) = v.value() {
                                if let Some(s) = resolve_str(&archive, *s_idx as usize) {
                                    all_event_selectors.insert(s);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(!all_classes.is_empty());
        assert!(!all_outlet_labels.is_empty());
        assert!(!all_event_selectors.is_empty());
        assert!(all_classes.contains("UITableView"));
        assert!(all_classes.contains("UITableViewCellContentView"));
        assert!(all_classes.contains("UIButton"));
        assert!(all_classes.contains("UILabel"));
        assert!(all_outlet_labels.contains("itemCell") || all_outlet_labels.contains("iconView"));
        assert!(all_event_selectors.contains("useItem"));
    }

    #[test]
    fn test_nib_instantiation_and_decoder_types() {
        use crate::frameworks::core_graphics::CGFloat;

        let mut env_inst = Environment::new_for_test().expect("create test environment");
        let env = &mut env_inst;

        // 1. Verify decode* methods on _touchHLE_NIBArchiveDecoder, NSKeyedUnarchiver, and NSCoder
        // to ensure msg! type IDs match between caller and callee without panic.
        let test_key = get_static_str(env, "NonExistentKey");

        // Test NSCoder base class
        let coder: id = msg_class![env; NSCoder alloc];
        let coder: id = msg![env; coder init];
        assert_ne!(coder, nil);
        let _: f64 = msg![env; coder decodeDoubleForKey:test_key];
        let _: f32 = msg![env; coder decodeFloatForKey:test_key];
        let _: bool = msg![env; coder decodeBoolForKey:test_key];
        let _: NSInteger = msg![env; coder decodeIntegerForKey:test_key];
        let _: i32 = msg![env; coder decodeIntForKey:test_key];
        let _: i32 = msg![env; coder decodeInt32ForKey:test_key];
        let _: i64 = msg![env; coder decodeInt64ForKey:test_key];
        let _: id = msg![env; coder decodeObjectForKey:test_key];
        let len_ptr: MutPtr<NSUInteger> = env.mem.alloc(4).cast();
        env.mem.write(len_ptr, 0);

        let _: CGPoint = msg![env; coder decodeCGPointForKey:test_key];
        let _: CGSize = msg![env; coder decodeCGSizeForKey:test_key];
        let _: CGRect = msg![env; coder decodeCGRectForKey:test_key];
        let _: ConstPtr<u8> = msg![env; coder decodeBytesForKey:test_key returnedLength:len_ptr];

        // Test NSKeyedUnarchiver
        let ku: id = msg_class![env; NSKeyedUnarchiver alloc];
        let ku: id = msg![env; ku init];
        assert_ne!(ku, nil);
        let _: f64 = msg![env; ku decodeDoubleForKey:test_key];
        let _: f32 = msg![env; ku decodeFloatForKey:test_key];
        let _: bool = msg![env; ku decodeBoolForKey:test_key];
        let _: NSInteger = msg![env; ku decodeIntegerForKey:test_key];
        let _: i32 = msg![env; ku decodeIntForKey:test_key];
        let _: i32 = msg![env; ku decodeInt32ForKey:test_key];
        let _: i64 = msg![env; ku decodeInt64ForKey:test_key];
        let _: id = msg![env; ku decodeObjectForKey:test_key];
        let _: CGPoint = msg![env; ku decodeCGPointForKey:test_key];
        let _: CGSize = msg![env; ku decodeCGSizeForKey:test_key];
        let _: CGRect = msg![env; ku decodeCGRectForKey:test_key];
        let _: ConstPtr<u8> = msg![env; ku decodeBytesForKey:test_key returnedLength:len_ptr];

        // Test _touchHLE_NIBArchiveDecoder
        let nib_decoder: id = msg_class![env; _touchHLE_NIBArchiveDecoder alloc];
        assert_ne!(nib_decoder, nil);
        let _: f64 = msg![env; nib_decoder decodeDoubleForKey:test_key];
        let _: f32 = msg![env; nib_decoder decodeFloatForKey:test_key];
        let _: bool = msg![env; nib_decoder decodeBoolForKey:test_key];
        let _: NSInteger = msg![env; nib_decoder decodeIntegerForKey:test_key];
        let _: i32 = msg![env; nib_decoder decodeIntForKey:test_key];
        let _: i32 = msg![env; nib_decoder decodeInt32ForKey:test_key];
        let _: i64 = msg![env; nib_decoder decodeInt64ForKey:test_key];
        let _: id = msg![env; nib_decoder decodeObjectForKey:test_key];
        let _: CGPoint = msg![env; nib_decoder decodeCGPointForKey:test_key];
        let _: CGSize = msg![env; nib_decoder decodeCGSizeForKey:test_key];
        let _: CGRect = msg![env; nib_decoder decodeCGRectForKey:test_key];
        let _: ConstPtr<u8> = msg![env; nib_decoder decodeBytesForKey:test_key returnedLength:len_ptr];

        env.mem.free(len_ptr.cast());

        // 2. Test full NIB instantiation on InventoryPanel.nib (which contains UITableView that crashed)
        let inv_bytes = std::fs::read("../touchHLE/touchHLE_apps/MapleCC.app/InventoryPanel.nib").unwrap_or_else(|_| {
            std::fs::read("../extracted/Payload/MapleCC.app/InventoryPanel.nib").expect("InventoryPanel.nib exists")
        });
        let ns_data: id = msg_class![env; NSData alloc];
        let inv_len = inv_bytes.len() as crate::mem::GuestUSize;
        let ptr = env.mem.alloc(inv_len);
        env.mem.bytes_at_mut(ptr.cast(), inv_len).copy_from_slice(&inv_bytes);
        let ns_data: id = msg![env; ns_data initWithBytesNoCopy:ptr length:(inv_len as NSUInteger) freeWhenDone:true];
        let inv_decoder: id = msg_class![env; _touchHLE_NIBArchiveDecoder alloc];
        let inv_decoder: id = msg![env; inv_decoder _touchHLE_initForReadingWithData:ns_data];
        assert_ne!(inv_decoder, nil);

        let objects_key = get_static_str(env, "UINibObjectsKey");
        let objects: id = msg![env; inv_decoder decodeObjectForKey:objects_key];
        assert_ne!(objects, nil);
        let obj_count: NSUInteger = msg![env; objects count];
        assert!(obj_count > 0, "InventoryPanel.nib must contain objects");

        // 3. Test full NIB instantiation on DropItemPanel.nib (which also contains UITableView for floor transitions)
        let drop_bytes = std::fs::read("../touchHLE/touchHLE_apps/MapleCC.app/DropItemPanel.nib").unwrap_or_else(|_| {
            std::fs::read("../extracted/Payload/MapleCC.app/DropItemPanel.nib").expect("DropItemPanel.nib exists")
        });
        let ns_data_drop: id = msg_class![env; NSData alloc];
        let drop_len = drop_bytes.len() as crate::mem::GuestUSize;
        let ptr_drop = env.mem.alloc(drop_len);
        env.mem.bytes_at_mut(ptr_drop.cast(), drop_len).copy_from_slice(&drop_bytes);
        let ns_data_drop: id = msg![env; ns_data_drop initWithBytesNoCopy:ptr_drop length:(drop_len as NSUInteger) freeWhenDone:true];
        let drop_decoder: id = msg_class![env; _touchHLE_NIBArchiveDecoder alloc];
        let drop_decoder: id = msg![env; drop_decoder _touchHLE_initForReadingWithData:ns_data_drop];
        assert_ne!(drop_decoder, nil);

        let drop_objects: id = msg![env; drop_decoder decodeObjectForKey:objects_key];
        assert_ne!(drop_objects, nil);
        let drop_count: NSUInteger = msg![env; drop_objects count];
        assert!(drop_count > 0, "DropItemPanel.nib must contain objects");

        // 4. Test UITableView initWithCoder: explicitly with coder
        let tv: id = msg_class![env; UITableView alloc];
        let tv: id = msg![env; tv initWithCoder:inv_decoder];
        assert_ne!(tv, nil);
        let row_h: CGFloat = msg![env; tv rowHeight];
        assert!(row_h > 0.0);

        // 5. Test NSNumber initWithCoder: on NS.realval / NS.dblval / NS.boolval / NS.intval
        // across NSKeyedUnarchiver, NIB archive decoder, and NSCoder
        // 5a. NSKeyedArchiver / NSKeyedUnarchiver roundtrip on double (NS.dblval)
        let num_d: id = msg_class![env; NSNumber alloc];
        let num_d: id = msg![env; num_d initWithDouble:123.456f64];
        let data_d: id = msg_class![env; NSKeyedArchiver archivedDataWithRootObject:num_d];
        assert_ne!(data_d, nil);
        let res_d: id = msg_class![env; NSKeyedUnarchiver unarchiveObjectWithData:data_d];
        assert_ne!(res_d, nil);
        let val_d: f64 = msg![env; res_d doubleValue];
        assert!((val_d - 123.456).abs() < 1e-4, "NSKeyedUnarchiver roundtrip double mismatch: {}", val_d);

        // 5b. NSKeyedArchiver / NSKeyedUnarchiver roundtrip on int64 (NS.intval)
        let num_i: id = msg_class![env; NSNumber alloc];
        let num_i: id = msg![env; num_i initWithLongLong:9876543210i64];
        let data_i: id = msg_class![env; NSKeyedArchiver archivedDataWithRootObject:num_i];
        assert_ne!(data_i, nil);
        let res_i: id = msg_class![env; NSKeyedUnarchiver unarchiveObjectWithData:data_i];
        assert_ne!(res_i, nil);
        let val_i: i64 = msg![env; res_i longLongValue];
        assert_eq!(val_i, 9876543210i64, "NSKeyedUnarchiver roundtrip int64 mismatch: {}", val_i);

        // 5c. NSKeyedArchiver / NSKeyedUnarchiver roundtrip on bool (NS.boolval)
        let num_b: id = msg_class![env; NSNumber alloc];
        let num_b: id = msg![env; num_b initWithBool:true];
        let data_b: id = msg_class![env; NSKeyedArchiver archivedDataWithRootObject:num_b];
        assert_ne!(data_b, nil);
        let res_b: id = msg_class![env; NSKeyedUnarchiver unarchiveObjectWithData:data_b];
        assert_ne!(res_b, nil);
        let val_b: bool = msg![env; res_b boolValue];
        assert_eq!(val_b, true, "NSKeyedUnarchiver roundtrip bool mismatch");

        // 5d. NSKeyedUnarchiver decoding NS.realval specifically
        let mut real_plist = plist::Dictionary::new();
        real_plist.insert("$archiver".into(), plist::Value::String("NSKeyedArchiver".into()));
        real_plist.insert("$version".into(), plist::Value::Integer(100000.into()));
        let mut top_dict = plist::Dictionary::new();
        top_dict.insert("root".into(), plist::Value::Uid(plist::Uid::new(1)));
        real_plist.insert("$top".into(), plist::Value::Dictionary(top_dict));

        let mut class_dict = plist::Dictionary::new();
        class_dict.insert("$classname".into(), plist::Value::String("NSNumber".into()));
        class_dict.insert("classes".into(), plist::Value::Array(vec![
            plist::Value::String("NSNumber".into()),
            plist::Value::String("NSValue".into()),
            plist::Value::String("NSObject".into()),
        ]));

        let mut num_dict = plist::Dictionary::new();
        num_dict.insert("$class".into(), plist::Value::Uid(plist::Uid::new(2)));
        num_dict.insert("NS.realval".into(), plist::Value::Real(45.75));

        real_plist.insert("$objects".into(), plist::Value::Array(vec![
            plist::Value::String("$null".into()),
            plist::Value::Dictionary(num_dict),
            plist::Value::Dictionary(class_dict),
        ]));

        let mut real_bytes = Vec::new();
        plist::to_writer_binary(&mut real_bytes, &plist::Value::Dictionary(real_plist)).expect("write binary plist");
        let real_data: id = msg_class![env; NSData alloc];
        let real_len = real_bytes.len() as crate::mem::GuestUSize;
        let real_ptr = env.mem.alloc(real_len);
        env.mem.bytes_at_mut(real_ptr.cast(), real_len).copy_from_slice(&real_bytes);
        let real_data: id = msg![env; real_data initWithBytesNoCopy:real_ptr length:(real_len as NSUInteger) freeWhenDone:true];
        let unarchived_real: id = msg_class![env; NSKeyedUnarchiver unarchiveObjectWithData:real_data];
        assert_ne!(unarchived_real, nil);
        let real_val: f64 = msg![env; unarchived_real doubleValue];
        assert!((real_val - 45.75).abs() < 1e-4, "NS.realval decode mismatch: {}", real_val);

        // 5e. Test NSNumber decoding from real NIB archive (BuyConfirmPanel.nib has NSNumber with NS.intval)
        let buy_bytes = std::fs::read("../touchHLE/touchHLE_apps/MapleCC.app/BuyConfirmPanel.nib").unwrap_or_else(|_| {
            std::fs::read("../extracted/Payload/MapleCC.app/BuyConfirmPanel.nib").expect("BuyConfirmPanel.nib exists")
        });
        let ns_data_buy: id = msg_class![env; NSData alloc];
        let buy_len = buy_bytes.len() as crate::mem::GuestUSize;
        let ptr_buy = env.mem.alloc(buy_len);
        env.mem.bytes_at_mut(ptr_buy.cast(), buy_len).copy_from_slice(&buy_bytes);
        let ns_data_buy: id = msg![env; ns_data_buy initWithBytesNoCopy:ptr_buy length:(buy_len as NSUInteger) freeWhenDone:true];
        let buy_decoder: id = msg_class![env; _touchHLE_NIBArchiveDecoder alloc];
        let buy_decoder: id = msg![env; buy_decoder _touchHLE_initForReadingWithData:ns_data_buy];
        assert_ne!(buy_decoder, nil);
        let num_idx = {
            let host_obj: &NIBArchiveDecoderHostObject = env.objc.borrow(buy_decoder);
            host_obj.archive.objects().iter().enumerate().find_map(|(idx, obj)| {
                if obj.class_name(host_obj.archive.class_names()).name() == "NSNumber" {
                    Some(idx as u32)
                } else {
                    None
                }
            })
        };
        assert!(num_idx.is_some(), "BuyConfirmPanel.nib archive must contain an NSNumber object");
        let unarchived_num = unarchive_obj(env, buy_decoder, num_idx.unwrap());
        assert_ne!(unarchived_num, nil);
        let _: NSInteger = msg![env; unarchived_num integerValue];
        let _: f64 = msg![env; unarchived_num doubleValue];
        let _: bool = msg![env; unarchived_num boolValue];

        // 5f. Direct initWithCoder: on base NSCoder
        let num3: id = msg_class![env; NSNumber alloc];
        let num3: id = msg![env; num3 initWithCoder:coder];
        assert_ne!(num3, nil);

        // 6. Test full UINib instantiateWithOwner:options: on InventoryPanel and DropItemPanel
        let owner: id = msg_class![env; NSObject alloc];
        let owner: id = msg![env; owner init];
        assert_ne!(owner, nil);

        let inv_name = get_static_str(env, "InventoryPanel");
        let inv_nib: id = msg_class![env; UINib nibWithNibName:inv_name bundle:nil];
        assert_ne!(inv_nib, nil);
        let inv_top: id = msg![env; inv_nib instantiateWithOwner:owner options:nil];
        assert_ne!(inv_top, nil);
        let inv_top_count: NSUInteger = msg![env; inv_top count];
        assert!(inv_top_count > 0, "InventoryPanel instantiateWithOwner should return top-level objects");

        let drop_name = get_static_str(env, "DropItemPanel");
        let drop_nib: id = msg_class![env; UINib nibWithNibName:drop_name bundle:nil];
        assert_ne!(drop_nib, nil);
        let drop_top: id = msg![env; drop_nib instantiateWithOwner:owner options:nil];
        assert_ne!(drop_top, nil);
        let drop_top_count: NSUInteger = msg![env; drop_top count];
        assert!(drop_top_count > 0, "DropItemPanel instantiateWithOwner should return top-level objects");

        // 7. Test NSBundle loadNibNamed:owner:options:
        let main_b: id = msg_class![env; NSBundle mainBundle];
        assert_ne!(main_b, nil);
        let loaded_inv: id = msg![env; main_b loadNibNamed:inv_name owner:owner options:nil];
        assert_ne!(loaded_inv, nil);
        let loaded_drop: id = msg![env; main_b loadNibNamed:drop_name owner:owner options:nil];
        assert_ne!(loaded_drop, nil);

        // 8. Test recursive viewWithTag:
        fn make_rect(x: f32, y: f32, w: f32, h: f32) -> CGRect {
            CGRect {
                origin: CGPoint { x, y },
                size: CGSize { width: w, height: h },
            }
        }

        let parent: id = msg_class![env; UIView alloc];
        let parent: id = msg![env; parent initWithFrame:(make_rect(0.0, 0.0, 100.0, 100.0))];
        let _: () = msg![env; parent setTag:(100 as NSInteger)];

        let child: id = msg_class![env; UIView alloc];
        let child: id = msg![env; child initWithFrame:(make_rect(0.0, 0.0, 50.0, 50.0))];
        let _: () = msg![env; child setTag:(101 as NSInteger)];

        let grandchild: id = msg_class![env; UIView alloc];
        let grandchild: id = msg![env; grandchild initWithFrame:(make_rect(0.0, 0.0, 20.0, 20.0))];
        let _: () = msg![env; grandchild setTag:(201 as NSInteger)];

        let _: () = msg![env; child addSubview:grandchild];
        let _: () = msg![env; parent addSubview:child];

        let found_parent: id = msg![env; parent viewWithTag:(100 as NSInteger)];
        assert_eq!(found_parent, parent);
        let found_child: id = msg![env; parent viewWithTag:(101 as NSInteger)];
        assert_eq!(found_child, child);
        let found_grandchild: id = msg![env; parent viewWithTag:(201 as NSInteger)];
        assert_eq!(found_grandchild, grandchild);
        let found_none: id = msg![env; parent viewWithTag:(999 as NSInteger)];
        assert_eq!(found_none, nil);

        // 9. Test clipsToBounds and masksToBounds
        let p_clips: bool = msg![env; parent clipsToBounds];
        assert_eq!(p_clips, false);
        let p_layer: id = msg![env; parent layer];
        let p_masks: bool = msg![env; p_layer masksToBounds];
        assert_eq!(p_masks, false);

        let _: () = msg![env; parent setClipsToBounds:true];
        let p_clips_after: bool = msg![env; parent clipsToBounds];
        assert_eq!(p_clips_after, true);
        let p_masks_after: bool = msg![env; p_layer masksToBounds];
        assert_eq!(p_masks_after, true);

        let scroll: id = msg_class![env; UIScrollView alloc];
        let scroll: id = msg![env; scroll initWithFrame:(make_rect(0.0, 0.0, 100.0, 100.0))];
        let scroll_clips: bool = msg![env; scroll clipsToBounds];
        assert_eq!(scroll_clips, true);

        // 10. Test UITableView multi-section selection and indexPathForSelectedRow
        let table: id = msg_class![env; UITableView alloc];
        let table: id = msg![env; table initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        let init_sel: id = msg![env; table indexPathForSelectedRow];
        assert_eq!(init_sel, nil);

        let ip: id = msg_class![env; NSIndexPath indexPathForRow:(3 as NSInteger) inSection:(2 as NSInteger)];
        assert_ne!(ip, nil);
        let ip_sec: NSInteger = msg![env; ip section];
        assert_eq!(ip_sec, 2);
        let ip_row: NSInteger = msg![env; ip row];
        assert_eq!(ip_row, 3);

        let _: () = msg![env; table selectRowAtIndexPath:ip animated:false scrollPosition:(0 as NSInteger)];
        let sel_row: id = msg![env; table indexPathForSelectedRow];
        assert_eq!(sel_row, ip);

        let _: () = msg![env; table deselectRowAtIndexPath:ip animated:false];
        let sel_row_after: id = msg![env; table indexPathForSelectedRow];
        assert_eq!(sel_row_after, nil);

        // 11. Test level-up skill selection NIBs unarchiving & recursive tag lookup
        let status_name = get_static_str(env, "StatusPanel");
        let status_nib: id = msg_class![env; UINib nibWithNibName:status_name bundle:nil];
        assert_ne!(status_nib, nil);
        let status_top: id = msg![env; status_nib instantiateWithOwner:owner options:nil];
        assert_ne!(status_top, nil);
        let status_count: NSUInteger = msg![env; status_top count];
        assert!(status_count > 0, "StatusPanel.nib should instantiate objects");

        let passive_name = get_static_str(env, "PassiveSkillsPane");
        let passive_nib: id = msg_class![env; UINib nibWithNibName:passive_name bundle:nil];
        assert_ne!(passive_nib, nil);
        let passive_top: id = msg![env; passive_nib instantiateWithOwner:owner options:nil];
        assert_ne!(passive_top, nil);
        let passive_count: NSUInteger = msg![env; passive_top count];
        assert!(passive_count > 0, "PassiveSkillsPane.nib should instantiate objects");

        let notice_name = get_static_str(env, "NoticePanel");
        let notice_nib: id = msg_class![env; UINib nibWithNibName:notice_name bundle:nil];
        assert_ne!(notice_nib, nil);
        let notice_top: id = msg![env; notice_nib instantiateWithOwner:owner options:nil];
        assert_ne!(notice_top, nil);
        let notice_count: NSUInteger = msg![env; notice_top count];
        assert!(notice_count > 0, "NoticePanel.nib should instantiate objects");

        let skill_name = get_static_str(env, "SkillCell");
        let skill_nib: id = msg_class![env; UINib nibWithNibName:skill_name bundle:nil];
        assert_ne!(skill_nib, nil);
        let skill_top: id = msg![env; skill_nib instantiateWithOwner:owner options:nil];
        assert_ne!(skill_top, nil);
        let skill_count: NSUInteger = msg![env; skill_top count];
        assert!(skill_count > 0, "SkillCell.nib should instantiate objects");

        // Find cell view
        let uiview_class: crate::objc::Class = msg_class![env; UIView class];
        let mut cell_view: id = nil;
        for i in 0..skill_count {
            let obj: id = msg![env; skill_top objectAtIndex:(i as NSUInteger)];
            if msg![env; obj isKindOfClass:uiview_class] {
                cell_view = obj;
                break;
            }
        }
        assert_ne!(cell_view, nil, "SkillCell.nib must instantiate a UIView object");

        // SkillCell description labels have tags 200, 201, 202 inside cell.contentView
        let desc0: id = msg![env; cell_view viewWithTag:(200 as NSInteger)];
        let desc1: id = msg![env; cell_view viewWithTag:(201 as NSInteger)];
        let desc2: id = msg![env; cell_view viewWithTag:(202 as NSInteger)];
        assert_ne!(desc0, nil, "SkillCell description label 200 must be found recursively");
        assert_ne!(desc1, nil, "SkillCell description label 201 must be found recursively");
        assert_ne!(desc2, nil, "SkillCell description label 202 must be found recursively");

        // 12. Test GKLocalPlayer singleton and authentication
        let player: id = msg_class![env; GKLocalPlayer localPlayer];
        assert_ne!(player, nil);
        let player2: id = msg_class![env; GKLocalPlayer localPlayer];
        assert_eq!(player, player2, "GKLocalPlayer localPlayer must return the singleton");
        let is_auth: bool = msg![env; player isAuthenticated];
        assert_eq!(is_auth, true);
        let alias: id = msg![env; player alias];
        assert_ne!(alias, nil);
        let pid: id = msg![env; player playerID];
        assert_ne!(pid, nil);
        let _: () = msg![env; player authenticateWithCompletionHandler:nil];

        // 13. Test SKProductsRequest
        let req: id = msg_class![env; SKProductsRequest alloc];
        let req: id = msg![env; req initWithProductIdentifiers:nil];
        assert_ne!(req, nil);
        let init_del: id = msg![env; req delegate];
        assert_eq!(init_del, nil);
        let _: () = msg![env; req setDelegate:owner];
        let set_del: id = msg![env; req delegate];
        assert_eq!(set_del, owner);
        let _: () = msg![env; req start];

        // 14. Test UIViewController title
        let vc: id = msg_class![env; UIViewController alloc];
        let vc: id = msg![env; vc init];
        assert_ne!(vc, nil);
        let vc_title: id = msg![env; vc title];
        assert_eq!(vc_title, nil);
        let test_title = get_static_str(env, "Skills");
        let _: () = msg![env; vc setTitle:test_title];
        let ret_title: id = msg![env; vc title];
        assert_eq!(ret_title, test_title);

        // 15. Test insertSubview:belowSubview: and insertSubview:aboveSubview: ordering (StatusPanel pattern)
        let v_container: id = msg_class![env; UIView alloc];
        let v_container: id = msg![env; v_container initWithFrame:(make_rect(0.0, 0.0, 200.0, 200.0))];
        let v_btn: id = msg_class![env; UIView alloc];
        let v_btn: id = msg![env; v_btn initWithFrame:(make_rect(0.0, 0.0, 50.0, 50.0))];
        let v_pane: id = msg_class![env; UIView alloc];
        let v_pane: id = msg![env; v_pane initWithFrame:(make_rect(0.0, 0.0, 150.0, 150.0))];
        let v_top_layer: id = msg_class![env; UIView alloc];
        let v_top_layer: id = msg![env; v_top_layer initWithFrame:(make_rect(0.0, 0.0, 30.0, 30.0))];

        let _: () = msg![env; v_container addSubview:v_btn];
        let _: () = msg![env; v_container insertSubview:v_pane belowSubview:v_btn];
        let _: () = msg![env; v_container insertSubview:v_top_layer aboveSubview:v_btn];

        let subviews: id = msg![env; v_container subviews];
        let sub_count: NSUInteger = msg![env; subviews count];
        assert_eq!(sub_count, 3);
        let s0: id = msg![env; subviews objectAtIndex:(0 as NSUInteger)];
        let s1: id = msg![env; subviews objectAtIndex:(1 as NSUInteger)];
        let s2: id = msg![env; subviews objectAtIndex:(2 as NSUInteger)];
        assert_eq!(s0, v_pane);
        assert_eq!(s1, v_btn);
        assert_eq!(s2, v_top_layer);

        // Edge case: inserting below nil appends safely
        let v_extra: id = msg_class![env; UIView alloc];
        let v_extra: id = msg![env; v_extra initWithFrame:(make_rect(0.0, 0.0, 10.0, 10.0))];
        let _: () = msg![env; v_container insertSubview:v_extra belowSubview:nil];
        let subviews2: id = msg![env; v_container subviews];
        let count2: NSUInteger = msg![env; subviews2 count];
        assert_eq!(count2, 4);

        // 16. Test performSelector:withObject:afterDelay: and cancellation
        let dummy_target: id = msg_class![env; NSObject alloc];
        let dummy_target: id = msg![env; dummy_target init];
        let dummy_sel = env.objc.register_host_selector("description".to_string(), &mut env.mem);
        let _: () = msg![env; dummy_target performSelector:dummy_sel withObject:nil afterDelay:10.0];
        let _: () = msg_class![env; NSObject cancelPreviousPerformRequestsWithTarget:dummy_target];

        // 17. Test CALayer convertRect:toLayer:nil for scissor rect computation
        let parent_layer: id = msg_class![env; CALayer alloc];
        let parent_layer: id = msg![env; parent_layer init];
        let p_frame = make_rect(0.0, 0.0, 320.0, 480.0);
        let _: () = msg![env; parent_layer setFrame:p_frame];

        let child_layer: id = msg_class![env; CALayer alloc];
        let child_layer: id = msg![env; child_layer init];
        let c_frame = make_rect(20.0, 100.0, 280.0, 300.0);
        let _: () = msg![env; child_layer setFrame:c_frame];
        let _: () = msg![env; parent_layer addSublayer:child_layer];

        let child_bounds: CGRect = msg![env; child_layer bounds];
        let cb_x = child_bounds.origin.x;
        let cb_y = child_bounds.origin.y;
        let cb_w = child_bounds.size.width;
        let cb_h = child_bounds.size.height;
        assert_eq!(cb_x, 0.0);
        assert_eq!(cb_y, 0.0);
        assert_eq!(cb_w, 280.0);
        assert_eq!(cb_h, 300.0);

        let screen_rect: CGRect = msg![env; child_layer convertRect:child_bounds toLayer:nil];
        let sr_x = screen_rect.origin.x;
        let sr_y = screen_rect.origin.y;
        let sr_w = screen_rect.size.width;
        let sr_h = screen_rect.size.height;
        assert_eq!(sr_x, 20.0, "convertRect:toLayer:nil x must be 20.0");
        assert_eq!(sr_y, 100.0, "convertRect:toLayer:nil y must be 100.0");
        assert_eq!(sr_w, 280.0);
        assert_eq!(sr_h, 300.0);

        // 18. Test UIButton setImage:forState: and subview ordering (background -> image -> label)
        let btn: id = msg_class![env; UIButton alloc];
        let btn: id = msg![env; btn initWithFrame:(make_rect(0.0, 0.0, 80.0, 40.0))];
        let btn_subs: id = msg![env; btn subviews];
        let btn_sub_count: NSUInteger = msg![env; btn_subs count];
        assert_eq!(btn_sub_count, 3, "UIButton must have 3 subviews");

        let bg_iv: id = msg![env; btn backgroundImageView];
        let iv: id = msg![env; btn imageView];
        let lbl: id = msg![env; btn titleLabel];
        let s_bg: id = msg![env; btn_subs objectAtIndex:(0 as NSUInteger)];
        let s_iv: id = msg![env; btn_subs objectAtIndex:(1 as NSUInteger)];
        let s_lbl: id = msg![env; btn_subs objectAtIndex:(2 as NSUInteger)];
        assert_eq!(s_bg, bg_iv, "First subview must be backgroundImageView");
        assert_eq!(s_iv, iv, "Second subview must be imageView");
        assert_eq!(s_lbl, lbl, "Third subview must be titleLabel");

        // 19. Test UITableView multi-section rectForRowAtIndexPath and indexPathsForVisibleRows
        let table_multi: id = msg_class![env; UITableView alloc];
        let table_multi: id = msg![env; table_multi initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        let cell_0_0: id = msg_class![env; UITableViewCell alloc];
        let cell_0_0: id = msg![env; cell_0_0 initWithFrame:(make_rect(0.0, 0.0, 320.0, 50.0))];
        let cell_1_0: id = msg_class![env; UITableViewCell alloc];
        let cell_1_0: id = msg![env; cell_1_0 initWithFrame:(make_rect(0.0, 50.0, 320.0, 60.0))];

        {
            let host = env.objc.borrow_mut::<crate::frameworks::uikit::ui_view::ui_scroll_view::ui_table_view::UITableViewHostObject>(table_multi);
            host.cells = vec![cell_0_0, cell_1_0];
            host.cell_index_paths = vec![(0, 0), (1, 0)];
        }

        let ip_sec1: id = msg_class![env; NSIndexPath indexPathForRow:(0 as NSInteger) inSection:(1 as NSInteger)];
        let rect_sec1: CGRect = msg![env; table_multi rectForRowAtIndexPath:ip_sec1];
        let r1_y = rect_sec1.origin.y;
        let r1_h = rect_sec1.size.height;
        assert_eq!(r1_y, 50.0, "Section 1 row 0 rect origin y must be 50.0");
        assert_eq!(r1_h, 60.0, "Section 1 row 0 rect height must be 60.0");

        let visible_ips: id = msg![env; table_multi indexPathsForVisibleRows];
        let vis_count: NSUInteger = msg![env; visible_ips count];
        assert_eq!(vis_count, 2);
        let first_ip: id = msg![env; visible_ips objectAtIndex:(0 as NSUInteger)];
        let second_ip: id = msg![env; visible_ips objectAtIndex:(1 as NSUInteger)];
        let f_sec: NSInteger = msg![env; first_ip section];
        let s_sec: NSInteger = msg![env; second_ip section];
        assert_eq!(f_sec, 0);
        assert_eq!(s_sec, 1, "Second visible index path must be section 1");

        // 20. Test UIViewController viewDidLoad invocation on pre-set view
        let test_vc: id = msg_class![env; UIViewController alloc];
        let test_vc: id = msg![env; test_vc init];
        let test_view: id = msg_class![env; UIView alloc];
        let test_view: id = msg![env; test_view initWithFrame:(make_rect(0.0, 0.0, 100.0, 100.0))];
        let _: () = msg![env; test_vc setView:test_view];
        assert!(msg![env; test_vc isViewLoaded]);
        let got_view: id = msg![env; test_vc view];
        assert_eq!(got_view, test_view);

        // 21. Test UITableView mock dataSource and zero recursion on reloadData, layoutIfNeeded, and layoutSubviews
        let ds: id = msg_class![env; _touchHLE_MockTableViewDataSource alloc];
        let ds: id = msg![env; ds init];
        assert_ne!(ds, nil, "Mock dataSource must be instantiated");

        let tv_test: id = msg_class![env; UITableView alloc];
        let tv_test: id = msg![env; tv_test initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        assert_ne!(tv_test, nil);

        // Setting dataSource should invoke reload_data without recursive loop
        let _: () = msg![env; tv_test setDataSource:ds];
        let subviews_tv: id = msg![env; tv_test subviews];
        let cell_count: NSUInteger = msg![env; subviews_tv count];
        assert_eq!(cell_count, 3, "UITableView with mock dataSource must create 3 cells");

        // Verify cell frames
        let c0: id = msg![env; subviews_tv objectAtIndex:(0 as NSUInteger)];
        let c0_frame: CGRect = msg![env; c0 frame];
        let c0_y = c0_frame.origin.y;
        let c0_h = c0_frame.size.height;
        assert_eq!(c0_y, 0.0);
        assert_eq!(c0_h, 44.0);

        let c1: id = msg![env; subviews_tv objectAtIndex:(1 as NSUInteger)];
        let c1_frame: CGRect = msg![env; c1 frame];
        let c1_y = c1_frame.origin.y;
        let c1_h = c1_frame.size.height;
        assert_eq!(c1_y, 44.0);
        assert_eq!(c1_h, 44.0);

        let c2: id = msg![env; subviews_tv objectAtIndex:(2 as NSUInteger)];
        let c2_frame: CGRect = msg![env; c2 frame];
        let c2_y = c2_frame.origin.y;
        let c2_h = c2_frame.size.height;
        assert_eq!(c2_y, 88.0);
        assert_eq!(c2_h, 44.0);

        // Calling layoutIfNeeded should complete cleanly with zero recursion
        let _: () = msg![env; tv_test layoutIfNeeded];

        // Calling reloadData should refresh data cleanly with zero recursion
        let _: () = msg![env; tv_test reloadData];
        let subviews_reloaded: id = msg![env; tv_test subviews];
        let reloaded_count: NSUInteger = msg![env; subviews_reloaded count];
        assert_eq!(reloaded_count, 3);

        // Calling layoutSubviews should complete with zero recursion
        let _: () = msg![env; tv_test layoutSubviews];

        // Calling setNeedsLayout then layoutIfNeeded
        let _: () = msg![env; tv_test setNeedsLayout];
        let _: () = msg![env; tv_test layoutIfNeeded];

        // Verify needs_layout is false after layoutIfNeeded
        let tv_needs_layout = env
            .objc
            .borrow::<crate::frameworks::uikit::ui_view::UIViewHostObject>(tv_test)
            .needs_layout;
        assert_eq!(tv_needs_layout, false, "needs_layout must be false after layoutIfNeeded");

        // 22. Test UITableView empty dataSource (0 rows): verify no infinite reload loop on layoutSubviews
        let empty_ds: id = msg_class![env; _touchHLE_MockEmptyTableViewDataSource alloc];
        let empty_ds: id = msg![env; empty_ds init];
        assert_ne!(empty_ds, nil);

        let tv_empty: id = msg_class![env; UITableView alloc];
        let tv_empty: id = msg![env; tv_empty initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        assert_ne!(tv_empty, nil);
        let _: () = msg![env; tv_empty setDataSource:empty_ds];
        let subviews_empty: id = msg![env; tv_empty subviews];
        let empty_count: NSUInteger = msg![env; subviews_empty count];
        assert_eq!(empty_count, 0, "Empty table view must have 0 cells");

        // Multiple calls to layoutSubviews should NOT trigger continuous reload_data
        for _ in 0..5 {
            let _: () = msg![env; tv_empty layoutSubviews];
            let _: () = msg![env; tv_empty layoutIfNeeded];
        }
        let tv_empty_needs_reload = env
            .objc
            .borrow::<crate::frameworks::uikit::ui_view::ui_scroll_view::ui_table_view::UITableViewHostObject>(tv_empty)
            .needs_reload;
        assert_eq!(tv_empty_needs_reload, false, "needs_reload must remain false for 0-row table");

        // 23. Test UITableView re-entrancy protection when dataSource calls reloadData during cellForRowAtIndexPath:
        let reentrant_ds: id = msg_class![env; _touchHLE_MockReentrantTableViewDataSource alloc];
        let reentrant_ds: id = msg![env; reentrant_ds init];
        assert_ne!(reentrant_ds, nil);

        let tv_reentrant: id = msg_class![env; UITableView alloc];
        let tv_reentrant: id = msg![env; tv_reentrant initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        assert_ne!(tv_reentrant, nil);
        let _: () = msg![env; tv_reentrant setDataSource:reentrant_ds];
        let subviews_reentrant: id = msg![env; tv_reentrant subviews];
        let reentrant_count: NSUInteger = msg![env; subviews_reentrant count];
        assert_eq!(reentrant_count, 2, "Re-entrant table view must have 2 cells without recursion");

        // 24. Test UITableView clean deallocation
        let tv_dealloc: id = msg_class![env; UITableView alloc];
        let tv_dealloc: id = msg![env; tv_dealloc initWithFrame:(make_rect(0.0, 0.0, 320.0, 480.0))];
        let _: () = msg![env; tv_dealloc setDataSource:ds];
        crate::objc::release(env, tv_dealloc);

        // 25. Test UIButton closePanel setHidden:true prevention
        let btn_normal: id = msg_class![env; UIButton alloc];
        let btn_normal: id = msg![env; btn_normal initWithFrame:(make_rect(0.0, 0.0, 100.0, 40.0))];
        assert_ne!(btn_normal, nil);
        let _: () = msg![env; btn_normal setHidden:true];
        let is_hidden_normal: bool = msg![env; btn_normal isHidden];
        assert_eq!(is_hidden_normal, true, "Normal button should be hidden when setHidden:true");

        let btn_close: id = msg_class![env; UIButton alloc];
        let btn_close: id = msg![env; btn_close initWithFrame:(make_rect(0.0, 0.0, 100.0, 40.0))];
        assert_ne!(btn_close, nil);
        let close_sel = env.objc.register_host_selector("closePanel".to_string(), &mut env.mem);
        let dummy_target: id = msg_class![env; NSObject alloc];
        let dummy_target: id = msg![env; dummy_target init];
        let _: () = msg![env; btn_close addTarget:dummy_target action:close_sel forControlEvents:(crate::frameworks::uikit::ui_view::ui_control::UIControlEventTouchUpInside)];
        let _: () = msg![env; btn_close setHidden:true];
        let is_hidden_close: bool = msg![env; btn_close isHidden];
        assert_eq!(is_hidden_close, false, "UIButton wired to closePanel must remain visible (isHidden == false) even when setHidden:true is called!");
        let _: () = msg![env; btn_close setHidden:false];
        let is_hidden_close_after: bool = msg![env; btn_close isHidden];
        assert_eq!(is_hidden_close_after, false, "UIButton wired to closePanel stays visible when setHidden:false");

        // 26. Test StatusPanel.nib instantiation and backButton preservation
        let status_bytes = std::fs::read("../touchHLE/touchHLE_apps/MapleCC.app/StatusPanel.nib").unwrap_or_else(|_| {
            std::fs::read("../extracted/Payload/MapleCC.app/StatusPanel.nib").expect("StatusPanel.nib exists")
        });
        let ns_data_status: id = msg_class![env; NSData alloc];
        let status_len = status_bytes.len() as crate::mem::GuestUSize;
        let ptr_status = env.mem.alloc(status_len);
        env.mem.bytes_at_mut(ptr_status.cast(), status_len).copy_from_slice(&status_bytes);
        let ns_data_status: id = msg![env; ns_data_status initWithBytesNoCopy:ptr_status length:(status_len as NSUInteger) freeWhenDone:true];
        let status_decoder: id = msg_class![env; _touchHLE_NIBArchiveDecoder alloc];
        let status_decoder: id = msg![env; status_decoder _touchHLE_initForReadingWithData:ns_data_status];
        assert_ne!(status_decoder, nil);
        let status_objects: id = msg![env; status_decoder decodeObjectForKey:objects_key];
        assert_ne!(status_objects, nil);
        let status_obj_count: NSUInteger = msg![env; status_objects count];
        assert!(status_obj_count > 0, "StatusPanel.nib must contain objects");
    }
}


