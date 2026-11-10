//! `UIGeometry.h`
//!
//! See also [crate::frameworks::core_graphics::cg_geometry].

use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string;
use crate::objc::{autorelease, id, nil};
use crate::Environment;

use crate::frameworks::core_graphics::cg_affine_transform::{CGAffineTransform, CGAffineTransformIdentity};

// Apple's documentation says all of these return zeroes/identity if the input is not
// well-formed.
pub fn CGPointFromString(env: &mut Environment, string: id) -> CGPoint {
    if string == nil {
        return CGPoint::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}
pub fn CGSizeFromString(env: &mut Environment, string: id) -> CGSize {
    if string == nil {
        return CGSize::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}
pub fn CGRectFromString(env: &mut Environment, string: id) -> CGRect {
    if string == nil {
        return CGRect::default();
    }
    // TODO: avoid copy
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or_default()
}

pub fn CGAffineTransformFromString(env: &mut Environment, string: id) -> CGAffineTransform {
    if string == nil {
        return CGAffineTransformIdentity;
    }
    ns_string::to_rust_string(env, string)
        .parse()
        .unwrap_or(CGAffineTransformIdentity)
}

pub fn NSStringFromCGPoint(env: &mut Environment, point: CGPoint) -> id {
    let s = ns_string::from_rust_string(env, point.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGSize(env: &mut Environment, size: CGSize) -> id {
    let s = ns_string::from_rust_string(env, size.to_string());
    autorelease(env, s)
}
pub fn NSStringFromCGRect(env: &mut Environment, rect: CGRect) -> id {
    let s = ns_string::from_rust_string(env, rect.to_string());
    autorelease(env, s)
}

pub fn NSStringFromCGAffineTransform(env: &mut Environment, transform: CGAffineTransform) -> id {
    let s = ns_string::from_rust_string(env, transform.to_string());
    autorelease(env, s)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGPointFromString(_)),
    export_c_func!(CGSizeFromString(_)),
    export_c_func!(CGRectFromString(_)),
    export_c_func!(CGAffineTransformFromString(_)),
    export_c_func!(NSStringFromCGPoint(_)),
    export_c_func!(NSStringFromCGSize(_)),
    export_c_func!(NSStringFromCGRect(_)),
    export_c_func!(NSStringFromCGAffineTransform(_)),
];

