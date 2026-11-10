/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImageView`.

use crate::frameworks::core_graphics::cg_image::CGImageRef;
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{NSInteger, NSTimeInterval, NSUInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_super, nil, objc_classes, release, retain,
    ClassExports, NSZonePtr,
};

#[derive(Default)]
struct UIImageViewHostObject {
    superclass: super::UIViewHostObject,
    /// `UIImage*`
    image: id,
    /// `NSArray<UIImage *>*`
    animation_images: id,
    animation_duration: NSTimeInterval,
    animation_repeat_count: NSInteger,
    is_animating: bool,
}
impl_HostObject_with_superclass!(UIImageViewHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIImageView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIImageViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];
    () = msg![env; this setUserInteractionEnabled:false];
    // Not sure if UIImageView does this unconditionally, or only for images
    // with alpha channels.
    () = msg![env; this setOpaque:false];
    this
}

- (())dealloc {
    let (image, anim_images) = {
        let host = env.objc.borrow::<UIImageViewHostObject>(this);
        (host.image, host.animation_images)
    };
    release(env, image);
    release(env, anim_images);
    msg_super![env; this dealloc]
}

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];

    let key_ns_string = get_static_str(env, "UIImage");
    let image: id = msg![env; coder decodeObjectForKey:key_ns_string];

    () = msg![env; this setImage:image];

    this
}

- (id)initWithImage:(id)image { // UIImage*
    let size: CGSize = msg![env; image size];
    let frame = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size
    };
    let this = msg_super![env; this initWithFrame:frame];
    () = msg![env; this setImage:image];
    // Not sure if UIImageView does this unconditionally, or only for images
    // with alpha channels.
    () = msg![env; this setOpaque:false];
    this
}

- (id)image {
    env.objc.borrow::<UIImageViewHostObject>(this).image
}

- (())setImage:(id)new_image { // UIImage*
    let host_obj = env.objc.borrow_mut::<UIImageViewHostObject>(this);
    let old_image = std::mem::replace(&mut host_obj.image, new_image);
    retain(env, new_image);
    release(env, old_image);

    let layer: id = msg![env; this layer];
    let cg_image: CGImageRef = if new_image != nil {
        msg![env; new_image CGImage]
    } else {
        nil
    };
    () = msg![env; layer setContents:cg_image];
}

- (())setAnimationImages:(id)images { // NSArray<UIImage *>*
    let host_obj = env.objc.borrow_mut::<UIImageViewHostObject>(this);
    let old = std::mem::replace(&mut host_obj.animation_images, images);
    retain(env, images);
    release(env, old);

    if images != nil {
        let count: NSUInteger = msg![env; images count];
        if count > 0 {
            let first_image: id = msg![env; images objectAtIndex:0u32];
            () = msg![env; this setImage:first_image];
        }
    }
}

- (id)animationImages {
    env.objc.borrow::<UIImageViewHostObject>(this).animation_images
}

- (())setAnimationDuration:(NSTimeInterval)duration {
    env.objc.borrow_mut::<UIImageViewHostObject>(this).animation_duration = duration;
}

- (NSTimeInterval)animationDuration {
    env.objc.borrow::<UIImageViewHostObject>(this).animation_duration
}

- (())setAnimationRepeatCount:(NSInteger)repeat_count {
    env.objc.borrow_mut::<UIImageViewHostObject>(this).animation_repeat_count = repeat_count;
}

- (NSInteger)animationRepeatCount {
    env.objc.borrow::<UIImageViewHostObject>(this).animation_repeat_count
}

- (bool)isAnimating {
    env.objc.borrow::<UIImageViewHostObject>(this).is_animating
}

- (())startAnimating {
    log_dbg!("[(UIImageView*) {:?} startAnimating]", this);
    env.objc.borrow_mut::<UIImageViewHostObject>(this).is_animating = true;
}

- (())stopAnimating {
    log_dbg!("[(UIImageView*) {:?} stopAnimating]", this);
    env.objc.borrow_mut::<UIImageViewHostObject>(this).is_animating = false;
}

- (())setHighlightedAnimationImages:(id)_images {
    // no-op
}

- (id)highlightedAnimationImages {
    nil
}

@end

};
