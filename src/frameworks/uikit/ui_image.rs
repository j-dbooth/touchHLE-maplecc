/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIImage`.

use crate::frameworks::core_graphics::cg_context::CGContextDrawImage;
use crate::frameworks::core_graphics::cg_image::{
    self, CGImageGetHeight, CGImageGetWidth, CGImageRef, CGImageRelease, CGImageRetain,
};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::{ns_data, ns_string, NSInteger};
use crate::frameworks::uikit::ui_graphics::UIGraphicsGetCurrentContext;
use crate::fs::GuestPath;
use crate::image::Image;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;
use std::collections::HashMap;

const CACHE_SIZE: usize = 60;

#[derive(Default)]
pub struct State {
    /// Cache of images for `[UIImage imageNamed:]` method.
    /// Images are explicitly retained.
    cached_images: HashMap<String, id>,
}
impl State {
    fn get(env: &Environment) -> &Self {
        &env.framework_state.uikit.ui_image
    }
    fn get_mut(env: &mut Environment) -> &mut Self {
        &mut env.framework_state.uikit.ui_image
    }
}

struct UIImageHostObject {
    cg_image: CGImageRef,
}
impl HostObject for UIImageHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIImage: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIImageHostObject { cg_image: nil });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)imageWithCGImage:(CGImageRef)cg_image {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithCGImage:cg_image];
    autorelease(env, new)
}

+ (id)imageNamed:(id)name { // NSString*
    // TODO: figure out whether this is actually correct in all cases
    let name_str = ns_string::to_rust_string(env, name).to_string();
    if let Some(&cached) = State::get(env).cached_images.get(&name_str) {
        return cached;
    }

    let bundle: id = msg_class![env; NSBundle mainBundle];
    let mut path: id = msg![env; bundle pathForResource:name ofType:nil];
    if path == nil {
        let candidates = if !name_str.contains('.') {
            vec![
                format!("{}.png", name_str),
                format!("{}@2x.png", name_str),
                format!("{}.PNG", name_str),
                format!("{}@2x.PNG", name_str),
                format!("{}.jpg", name_str),
                format!("{}@2x.jpg", name_str),
                format!("{}.jpeg", name_str),
            ]
        } else if let Some((stem, ext)) = name_str.rsplit_once('.') {
            vec![
                format!("{}@2x.{}", stem, ext),
            ]
        } else {
            Vec::new()
        };

        for candidate in candidates {
            let cand_ns = ns_string::from_rust_string(env, candidate);
            path = msg![env; bundle pathForResource:cand_ns ofType:nil];
            release(env, cand_ns);
            if path != nil {
                break;
            }
        }
    }

    if path == nil {
        log!("Warning: [UIImage imageNamed:{:?}] => nil", name_str);
        return nil;
    }
    // TODO: find a better eviction policy
    if State::get(env).cached_images.len() > CACHE_SIZE {
        let cache = std::mem::take(&mut State::get_mut(env).cached_images);
        log_dbg!("Evicting {} images from UIImage cache.", cache.len());
        for (_, img) in cache {
            release(env, img);
        }
    }
    let img = msg![env; this imageWithContentsOfFile:path];
    if img != nil {
        retain(env, img);
        State::get_mut(env).cached_images.insert(name_str.clone(), img);
    }
    img
}

+ (id)imageWithContentsOfFile:(id)path { // NSString*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithContentsOfFile:path];
    autorelease(env, new)
}

+ (id)imageWithData:(id)data { // NSData*
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithData:data];
    autorelease(env, new)
}

- (())dealloc {
    let &UIImageHostObject { cg_image } = env.objc.borrow(this);
    CGImageRelease(env, cg_image);

    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)initWithCGImage:(CGImageRef)cg_image {
    CGImageRetain(env, cg_image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)initWithContentsOfFile:(id)path { // NSString*
    if path == nil {
        return nil;
    }
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    let Ok(bytes) = env.fs.read(GuestPath::new(&path)) else {
        log_dbg!("Warning: couldn't read image file at {:?}, returning nil", path);
        release(env, this);
        return nil;
    };
    let image = match Image::from_bytes(&bytes) {
        Ok(image) => image,
        Err(e) => {
            log!("Warning: couldn't decode image {:?}: {}, returning nil", path, e);
            release(env, this);
            return nil;
        }
    };
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)initWithData:(id)data { // NSData*
    let slice = ns_data::to_rust_slice(env, data);
    let image = match Image::from_bytes(slice) {
        Ok(image) => image,
        Err(e) => {
            log!("Warning: [UIImage initWithData:] couldn't decode image (len={}): {}, returning nil", slice.len(), e);
            release(env, this);
            return nil;
        }
    };
    let cg_image = cg_image::from_image(env, image);
    env.objc.borrow_mut::<UIImageHostObject>(this).cg_image = cg_image;
    this
}

- (id)stretchableImageWithLeftCapWidth:(NSInteger)_leftCapWidth
                          topCapHeight:(NSInteger)_topCapHeight {
    log!("TODO: properly support stretchableImageWithLeftCapWidth:topCapHeight:");
    retain(env, this)
}

// TODO: more init methods
// TODO: more accessors

- (CGImageRef)CGImage {
    env.objc.borrow::<UIImageHostObject>(this).cg_image
}

// TODO: should have UIImageOrientation type
- (NSInteger)imageOrientation {
    // FIXME: load image orientation info from file?
    0 // UIImageOrientationUp
}

- (CGSize)size {
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    let (width, height) = cg_image::borrow_image(&env.objc, image).dimensions();
    CGSize {
        width: width as _,
        height: height as _,
    }
}

- (CGFloat)scale {
    // TODO: support other scales, such as @2x
    1.0
}

- (())drawInRect:(CGRect)rect {
    let context = UIGraphicsGetCurrentContext(env);
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    CGContextDrawImage(env, context, rect, image);
}

- (())drawAtPoint:(CGPoint)point {
    let context = UIGraphicsGetCurrentContext(env);
    if context == nil {
        log!("Warning: [(UIImage*){:?} drawAtPoint:{:?}] is called with nil context, ignoring.", this, point);
        return;
    }
    let image = env.objc.borrow::<UIImageHostObject>(this).cg_image;
    let rect = CGRect {
        origin: point,
        size: CGSize {
            width: CGImageGetWidth(env, image) as CGFloat,
            height: CGImageGetHeight(env, image) as CGFloat,
        }
    };
    CGContextDrawImage(env, context, rect, image);
}

@end

// Undocumented class used in NIBs
// TODO: It's not clear _why_ placeholder is needed?
@implementation UIImageNibPlaceholder: UIImage

// NSCoding implementation
- (id)initWithCoder:(id)coder {
    release(env, this);

    // TODO: decode other attributes
    let key_ns_string = get_static_str(env, "UIResourceName");
    let resource_name: id = msg![env; coder decodeObjectForKey:key_ns_string];

    let res = msg_class![env; UIImage imageNamed:resource_name];
    // TODO: It is not clear if we need to additionally retain here?
    retain(env, res)
}

@end

};

pub fn UIImagePNGRepresentation(env: &mut Environment, image: id) -> id {
    if image == nil {
        return nil;
    }
    let &UIImageHostObject { cg_image } = env.objc.borrow(image);
    if cg_image == nil {
        return nil;
    }
    let img = cg_image::borrow_image(&env.objc, cg_image);
    let (width, height) = img.dimensions();
    let pixels = img.pixels();
    let png_bytes = crate::image::encode_rgba_to_png(width, height, pixels, false);
    ns_data::from_rust_slice(env, &png_bytes)
}

pub fn UIImageJPEGRepresentation(env: &mut Environment, image: id, _compression_quality: CGFloat) -> id {
    UIImagePNGRepresentation(env, image)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(UIImagePNGRepresentation(_)),
    export_c_func!(UIImageJPEGRepresentation(_, _)),
];

