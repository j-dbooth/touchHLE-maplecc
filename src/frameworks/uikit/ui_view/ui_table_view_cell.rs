/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITableViewCell` and `UITableViewCellContentView`.

use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::NSInteger;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};

pub struct UITableViewCellHostObject {
    superclass: super::UIViewHostObject,
    content_view: id,
    text_label: id,
    detail_text_label: id,
    image_view: id,
    pub reuse_identifier: id,
    selection_style: NSInteger,
    selected: bool,
    highlighted: bool,
}
impl_HostObject_with_superclass!(UITableViewCellHostObject);
impl Default for UITableViewCellHostObject {
    fn default() -> Self {
        Self {
            superclass: Default::default(),
            content_view: nil,
            text_label: nil,
            detail_text_label: nil,
            image_view: nil,
            reuse_identifier: nil,
            selection_style: 1, // UITableViewCellSelectionStyleBlue
            selected: false,
            highlighted: false,
        }
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITableViewCell: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITableViewCellHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];
    let cv: id = msg_class![env; UITableViewCellContentView alloc];
    let cv: id = msg![env; cv initWithFrame:frame];
    () = msg![env; this addSubview:cv];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).content_view = cv;
    this
}

- (id)initWithStyle:(NSInteger)_style reuseIdentifier:(id)reuseIdentifier {
    retain(env, reuseIdentifier);
    let rect = CGRect::default();
    let this: id = msg![env; this initWithFrame:rect];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).reuse_identifier = reuseIdentifier;
    this
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let subviews: id = msg![env; this subviews];
    let count: crate::frameworks::foundation::NSUInteger = msg![env; subviews count];
    let content_view_class = env.objc.get_known_class("UITableViewCellContentView", &mut env.mem);
    let mut found_cv = nil;
    for i in 0..count {
        let subview: id = msg![env; subviews objectAtIndex:i];
        if msg![env; subview isKindOfClass:content_view_class] {
            found_cv = subview;
            break;
        }
    }
    if found_cv == nil {
        let key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIContentView");
        let cv: id = msg![env; coder decodeObjectForKey:key];
        if cv != nil {
            let _: () = msg![env; this addSubview:cv];
            found_cv = cv;
        }
    }
    if found_cv == nil {
        let bounds: CGRect = msg![env; this bounds];
        let cv: id = msg_class![env; UITableViewCellContentView alloc];
        let cv: id = msg![env; cv initWithFrame:bounds];
        () = msg![env; this addSubview:cv];
        found_cv = cv;
    }
    if found_cv != nil {
        let bounds: CGRect = msg![env; this bounds];
        if bounds.size.width > 0.0 && bounds.size.height > 0.0 {
            let _: () = msg![env; found_cv setFrame:bounds];
        }
    }
    let reuse_id_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UIReuseIdentifier");
    let reuse_id: id = msg![env; coder decodeObjectForKey:reuse_id_key];
    if reuse_id != nil {
        retain(env, reuse_id);
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).reuse_identifier = reuse_id;
    }
    let selection_style_key = crate::frameworks::foundation::ns_string::get_static_str(env, "UISelectionStyle");
    if msg![env; coder containsValueForKey:selection_style_key] {
        let style: NSInteger = msg![env; coder decodeIntegerForKey:selection_style_key];
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).selection_style = style;
    }

    env.objc.borrow_mut::<UITableViewCellHostObject>(this).content_view = found_cv;
    this
}

- (())setFrame:(CGRect)frame {
    let _: () = msg_super![env; this setFrame:frame];
    let cv = env.objc.borrow::<UITableViewCellHostObject>(this).content_view;
    if cv != nil {
        let bounds: CGRect = msg![env; this bounds];
        let _: () = msg![env; cv setFrame:bounds];
    }
}

- (())setBounds:(CGRect)bounds {
    let _: () = msg_super![env; this setBounds:bounds];
    let cv = env.objc.borrow::<UITableViewCellHostObject>(this).content_view;
    if cv != nil {
        let b: CGRect = msg![env; this bounds];
        let _: () = msg![env; cv setFrame:b];
    }
}

- (())layoutSubviews {
    let _: () = msg_super![env; this layoutSubviews];
    let cv = env.objc.borrow::<UITableViewCellHostObject>(this).content_view;
    if cv != nil {
        let bounds: CGRect = msg![env; this bounds];
        let _: () = msg![env; cv setFrame:bounds];
    }
}

- (id)contentView {
    let cv = env.objc.borrow::<UITableViewCellHostObject>(this).content_view;
    if cv != nil {
        cv
    } else {
        let bounds: CGRect = msg![env; this bounds];
        let new_cv: id = msg_class![env; UITableViewCellContentView alloc];
        let new_cv: id = msg![env; new_cv initWithFrame:bounds];
        () = msg![env; this addSubview:new_cv];
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).content_view = new_cv;
        new_cv
    }
}

- (id)textLabel {
    let label = env.objc.borrow::<UITableViewCellHostObject>(this).text_label;
    if label != nil {
        label
    } else {
        let new_label: id = msg_class![env; UILabel alloc];
        let rect = CGRect::default();
        let new_label: id = msg![env; new_label initWithFrame:rect];
        let cv = msg![env; this contentView];
        () = msg![env; cv addSubview:new_label];
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).text_label = new_label;
        new_label
    }
}

- (id)detailTextLabel {
    let label = env.objc.borrow::<UITableViewCellHostObject>(this).detail_text_label;
    if label != nil {
        label
    } else {
        let new_label: id = msg_class![env; UILabel alloc];
        let rect = CGRect::default();
        let new_label: id = msg![env; new_label initWithFrame:rect];
        let cv = msg![env; this contentView];
        () = msg![env; cv addSubview:new_label];
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).detail_text_label = new_label;
        new_label
    }
}

- (id)imageView {
    let iv = env.objc.borrow::<UITableViewCellHostObject>(this).image_view;
    if iv != nil {
        iv
    } else {
        let new_iv: id = msg_class![env; UIImageView alloc];
        let rect = CGRect::default();
        let new_iv: id = msg![env; new_iv initWithFrame:rect];
        let cv = msg![env; this contentView];
        () = msg![env; cv addSubview:new_iv];
        env.objc.borrow_mut::<UITableViewCellHostObject>(this).image_view = new_iv;
        new_iv
    }
}

- (id)reuseIdentifier {
    env.objc.borrow::<UITableViewCellHostObject>(this).reuse_identifier
}

- (NSInteger)selectionStyle {
    env.objc.borrow::<UITableViewCellHostObject>(this).selection_style
}

- (())setSelectionStyle:(NSInteger)style {
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).selection_style = style;
}

- (())setSelected:(bool)selected animated:(bool)_animated {
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).selected = selected;
}

- (())setSelected:(bool)selected {
    () = msg![env; this setSelected:selected animated:false];
}

- (bool)isSelected {
    env.objc.borrow::<UITableViewCellHostObject>(this).selected
}

- (())setHighlighted:(bool)highlighted animated:(bool)_animated {
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).highlighted = highlighted;
}

- (())setHighlighted:(bool)highlighted {
    () = msg![env; this setHighlighted:highlighted animated:false];
}

- (bool)isHighlighted {
    env.objc.borrow::<UITableViewCellHostObject>(this).highlighted
}

- (id)backgroundView { nil }
- (())setBackgroundView:(id)_view {}
- (id)selectedBackgroundView { nil }
- (())setSelectedBackgroundView:(id)_view {}

- (NSInteger)accessoryType { 0 }
- (())setAccessoryType:(NSInteger)_type {}
- (id)accessoryView { nil }
- (())setAccessoryView:(id)_view {}

- (())prepareForReuse {
    // no-op
}

- (())dealloc {
    let &UITableViewCellHostObject {
        content_view,
        text_label,
        detail_text_label,
        image_view,
        reuse_identifier,
        ..
    } = env.objc.borrow(this);
    release(env, content_view);
    release(env, text_label);
    release(env, detail_text_label);
    release(env, image_view);
    release(env, reuse_identifier);
    msg_super![env; this dealloc]
}

@end

@implementation UITableViewCellContentView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<super::UIViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    msg_super![env; this initWithFrame:frame]
}

- (id)initWithCoder:(id)coder {
    msg_super![env; this initWithCoder:coder]
}

@end

};
