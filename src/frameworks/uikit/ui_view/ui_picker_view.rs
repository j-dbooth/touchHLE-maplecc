/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIPickerView`.

use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, todo_objc_setter,
    ClassExports, NSZonePtr,
};

// TODO: rendering

struct UIPickerViewHostObject {
    superclass: super::UIViewHostObject,
    delegate: id,
    data_source: id,
    shows_selection_indicator: bool,
}
impl_HostObject_with_superclass!(UIPickerViewHostObject);
impl Default for UIPickerViewHostObject {
    fn default() -> Self {
        Self {
            superclass: Default::default(),
            delegate: nil,
            data_source: nil,
            shows_selection_indicator: false,
        }
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIPickerView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIPickerViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let del_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIDelegate");
    let del: id = msg![env; coder decodeObjectForKey:del_key];
    if del != nil {
        () = msg![env; this setDelegate:del];
    }
    let ds_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIDataSource");
    let ds: id = msg![env; coder decodeObjectForKey:ds_key];
    if ds != nil {
        () = msg![env; this setDataSource:ds];
    }
    this
}

- (())setShowsSelectionIndicator:(bool)shows {
    todo_objc_setter!(this, shows);
    env.objc.borrow_mut::<UIPickerViewHostObject>(this).shows_selection_indicator = shows;
}
- (bool)showsSelectionIndicator {
    env.objc.borrow::<UIPickerViewHostObject>(this).shows_selection_indicator
}

- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UIPickerViewHostObject>(this).delegate = delegate;
}
- (id)delegate {
    env.objc.borrow::<UIPickerViewHostObject>(this).delegate
}

- (())setDataSource:(id)dataSource {
    env.objc.borrow_mut::<UIPickerViewHostObject>(this).data_source = dataSource;
}
- (id)dataSource {
    env.objc.borrow::<UIPickerViewHostObject>(this).data_source
}

- (NSInteger)selectedRowInComponent:(NSInteger)_component {
    0
}

- (())reloadComponent:(NSInteger)_component {
    // no-op
}

- (())reloadAllComponents {
    // no-op
}

@end

};
