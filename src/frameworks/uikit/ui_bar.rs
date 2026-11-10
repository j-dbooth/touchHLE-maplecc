/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIBarItem`, `UIBarButtonItem`, `UITabBarItem`, `UITabBar`, and `UIToolbar`.

use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    id, msg, msg_class, msg_super, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr, SEL,
};

struct UIBarItemHostObject {
    tag: NSInteger,
    enabled: bool,
    title: id,
    image: id,
}
impl HostObject for UIBarItemHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIBarItem: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIBarItemHostObject {
        tag: 0,
        enabled: true,
        title: nil,
        image: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let title_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UITitle");
    let title: id = msg![env; coder decodeObjectForKey:title_key];
    if title != nil {
        () = msg![env; this setTitle:title];
    }
    let img_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIImage");
    let img: id = msg![env; coder decodeObjectForKey:img_key];
    if img != nil {
        () = msg![env; this setImage:img];
    }
    let tag_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UITag");
    let tag: NSInteger = msg![env; coder decodeIntegerForKey:tag_key];
    env.objc.borrow_mut::<UIBarItemHostObject>(this).tag = tag;
    let enabled_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIEnabled");
    if msg![env; coder containsValueForKey:enabled_key] {
        let enabled: bool = msg![env; coder decodeBoolForKey:enabled_key];
        env.objc.borrow_mut::<UIBarItemHostObject>(this).enabled = enabled;
    }
    this
}

- (NSInteger)tag {
    env.objc.borrow::<UIBarItemHostObject>(this).tag
}

- (())setTag:(NSInteger)tag {
    env.objc.borrow_mut::<UIBarItemHostObject>(this).tag = tag;
}

- (bool)isEnabled {
    env.objc.borrow::<UIBarItemHostObject>(this).enabled
}

- (())setEnabled:(bool)enabled {
    env.objc.borrow_mut::<UIBarItemHostObject>(this).enabled = enabled;
}

- (id)title {
    env.objc.borrow::<UIBarItemHostObject>(this).title
}

- (())setTitle:(id)title {
    retain(env, title);
    let old = std::mem::replace(
        &mut env.objc.borrow_mut::<UIBarItemHostObject>(this).title,
        title,
    );
    release(env, old);
}

- (id)image {
    env.objc.borrow::<UIBarItemHostObject>(this).image
}

- (())setImage:(id)image {
    retain(env, image);
    let old = std::mem::replace(
        &mut env.objc.borrow_mut::<UIBarItemHostObject>(this).image,
        image,
    );
    release(env, old);
}

- (())dealloc {
    let &UIBarItemHostObject { title, image, .. } = env.objc.borrow(this);
    release(env, title);
    release(env, image);
    env.objc.dealloc_object(this, &mut env.mem);
}

@end

@implementation UIBarButtonItem: UIBarItem

- (id)initWithTitle:(id)title style:(NSInteger)_style target:(id)_target action:(SEL)_action {
    let this: id = msg_super![env; this init];
    () = msg![env; this setTitle:title];
    this
}

- (id)initWithImage:(id)image style:(NSInteger)_style target:(id)_target action:(SEL)_action {
    let this: id = msg_super![env; this init];
    () = msg![env; this setImage:image];
    this
}

- (id)initWithBarButtonSystemItem:(NSInteger)_item target:(id)_target action:(SEL)_action {
    msg_super![env; this init]
}

@end

@implementation UITabBarItem: UIBarItem

- (id)initWithTitle:(id)title image:(id)image tag:(NSInteger)tag {
    let this: id = msg_super![env; this init];
    () = msg![env; this setTitle:title];
    () = msg![env; this setImage:image];
    () = msg![env; this setTag:tag];
    this
}

- (id)initWithTabBarSystemItem:(NSInteger)_item tag:(NSInteger)tag {
    let this: id = msg_super![env; this init];
    () = msg![env; this setTag:tag];
    this
}

@end

@implementation UIToolbar: UIView

- (id)initWithFrame:(CGRect)frame {
    msg_super![env; this initWithFrame:frame]
}

- (id)initWithCoder:(id)coder {
    msg_super![env; this initWithCoder:coder]
}

- (())setItems:(id)_items animated:(bool)_animated {
    // no-op
}

- (())setItems:(id)_items {
    // no-op
}

- (id)items {
    msg_class![env; NSArray array]
}

- (NSInteger)barStyle {
    0
}

- (())setBarStyle:(NSInteger)_style {
    // no-op
}

@end

@implementation UITabBar: UIView

- (id)initWithFrame:(CGRect)frame {
    msg_super![env; this initWithFrame:frame]
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let del_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIDelegate");
    let del: id = msg![env; coder decodeObjectForKey:del_key];
    if del != nil {
        () = msg![env; this setDelegate:del];
    }
    let items_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIItems");
    let items: id = msg![env; coder decodeObjectForKey:items_key];
    if items != nil {
        () = msg![env; this setItems:items];
    }
    this
}

- (())setItems:(id)_items animated:(bool)_animated {
    // no-op
}

- (())setItems:(id)_items {
    // no-op
}

- (id)items {
    msg_class![env; NSArray array]
}

- (id)selectedItem {
    nil
}

- (())setSelectedItem:(id)_item {
    // no-op
}

- (id)delegate {
    nil
}

- (())setDelegate:(id)_delegate {
    // no-op
}

@end

};
