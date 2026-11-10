/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CGBitmapContext.h`

use super::cg_affine_transform::{CGAffineTransform, CGAffineTransformIdentity};
use super::cg_color_space::{
    kCGColorSpaceGenericGray, kCGColorSpaceGenericRGB, CGColorSpaceHostObject, CGColorSpaceRef,
};
use super::cg_context::{
    kCGBlendModeCopy, kCGBlendModeDarken, kCGBlendModeLighten, kCGBlendModeMultiply,
    kCGBlendModeNormal, kCGBlendModeScreen, CGBlendMode, CGContextHostObject, CGContextRef,
    CGContextSubclass,
};
use super::cg_image::{
    self, kCGBitmapAlphaInfoMask, kCGBitmapByteOrderMask, kCGImageAlphaFirst, kCGImageAlphaLast,
    kCGImageAlphaNone, kCGImageAlphaNoneSkipFirst, kCGImageAlphaNoneSkipLast, kCGImageAlphaOnly,
    kCGImageAlphaPremultipliedFirst, kCGImageAlphaPremultipliedLast, kCGImageByteOrder32Big,
    kCGImageByteOrder32Little, kCGImageByteOrderDefault, CGBitmapInfo, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGImageRef,
};
use super::{CGFloat, CGPoint, CGRect};
use crate::dyld::{export_c_func, FunctionExports};
use crate::image::{gamma_decode, gamma_encode, Image};
use crate::mem::{GuestUSize, Mem, MutVoidPtr, Ptr};
use crate::objc::{nil, ObjC};
use crate::Environment;

#[derive(Copy, Clone)]
pub(super) struct CGBitmapContextData {
    pub(super) data: MutVoidPtr,
    pub(super) data_is_owned: bool,
    width: GuestUSize,
    height: GuestUSize,
    bits_per_component: GuestUSize,
    bytes_per_row: GuestUSize,
    color_space: &'static str,
    alpha_info: CGImageAlphaInfo,
    byte_order: CGImageByteOrderInfo,
}

pub fn CGBitmapContextCreate(
    env: &mut Environment,
    data: MutVoidPtr,
    width: GuestUSize,
    height: GuestUSize,
    bits_per_component: GuestUSize,
    bytes_per_row: GuestUSize,
    color_space: CGColorSpaceRef,
    bitmap_info: u32,
) -> CGContextRef {
    assert!(bits_per_component == 8); // TODO: support other bit depths

    if width == 0 || height == 0 {
        return nil;
    }

    let color_space = if color_space.is_null() {
        if (bitmap_info & kCGBitmapAlphaInfoMask) == kCGImageAlphaOnly {
            kCGColorSpaceGenericGray
        } else {
            return nil;
        }
    } else {
        env.objc.borrow::<CGColorSpaceHostObject>(color_space).name
    };

    let component_count = match color_space {
        kCGColorSpaceGenericRGB => match components_for_rgb(bitmap_info) {
            Ok(count) => count,
            Err(()) => return nil,
        },
        kCGColorSpaceGenericGray => match components_for_gray(bitmap_info) {
            Ok(count) => count,
            Err(()) => return nil,
        },
        _ => unimplemented!("support other color spaces"),
    };

    let (data, data_is_owned, bytes_per_row) = if data.is_null() {
        let bytes_per_row = if bytes_per_row == 0 {
            width.checked_mul(component_count).unwrap()
        } else {
            bytes_per_row
        };
        let total_size = bytes_per_row.checked_mul(height).unwrap();
        let data = env.mem.alloc(total_size);
        (data, true, bytes_per_row)
    } else {
        assert!(bytes_per_row != 0);
        (data, false, bytes_per_row)
    };

    let host_object = CGContextHostObject {
        subclass: CGContextSubclass::CGBitmapContext(CGBitmapContextData {
            data,
            data_is_owned,
            width,
            height,
            bits_per_component,
            bytes_per_row,
            color_space,
            alpha_info: bitmap_info & kCGBitmapAlphaInfoMask,
            byte_order: bitmap_info & kCGBitmapByteOrderMask,
        }),
        // TODO: is this the correct default?
        rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
        font: Ptr::null(),
        font_size: 14.0,
        transform: CGAffineTransformIdentity,
        blend_mode: kCGBlendModeNormal,
        text_transform: None,
        state_stack: Vec::new(),
    };
    let isa = env
        .objc
        .get_known_class("_touchHLE_CGContext", &mut env.mem);
    env.objc
        .alloc_object(isa, Box::new(host_object), &mut env.mem)
}

fn CGBitmapContextGetData(env: &mut Environment, context: CGContextRef) -> MutVoidPtr {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.data
}

pub fn CGBitmapContextGetWidth(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.width
}

pub fn CGBitmapContextGetHeight(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.height
}

fn CGBitmapContextGetBytesPerRow(env: &mut Environment, context: CGContextRef) -> GuestUSize {
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    bitmap_data.bytes_per_row
}

pub(super) fn convert_bitmap_context_pixels_to_rgba(
    data: &CGBitmapContextData,
    raw_bytes: &[u8],
) -> Vec<u8> {
    let bpp = bytes_per_pixel(data) as usize;
    let offsets = pixel_offsets(data);
    let mut pixels = Vec::with_capacity((data.width * data.height * 4) as usize);
    for y in 0..data.height {
        let row_start = (y * data.bytes_per_row) as usize;
        for x in 0..data.width {
            let pixel_idx = row_start + (x as usize) * bpp;
            let (r, g, b, a) = if data.alpha_info == kCGImageAlphaOnly {
                (0, 0, 0, raw_bytes[pixel_idx])
            } else {
                let r = raw_bytes[pixel_idx + offsets.0];
                let g = raw_bytes[pixel_idx + offsets.1];
                let b = raw_bytes[pixel_idx + offsets.2];
                let a = if let Some(a_offset) = offsets.3 {
                    raw_bytes[pixel_idx + a_offset]
                } else {
                    0xFF
                };
                let (r, g, b) = if matches!(
                    data.alpha_info,
                    kCGImageAlphaLast | kCGImageAlphaFirst
                ) && a != 0xFF
                {
                    let a_factor = a as f32 / 255.0;
                    (
                        (r as f32 * a_factor).round() as u8,
                        (g as f32 * a_factor).round() as u8,
                        (b as f32 * a_factor).round() as u8,
                    )
                } else {
                    (r, g, b)
                };
                (r, g, b, a)
            };
            pixels.extend_from_slice(&[r, g, b, a]);
        }
    }
    pixels
}

pub fn CGBitmapContextCreateImage(env: &mut Environment, context: CGContextRef) -> CGImageRef {
    // TODO: Image::from_pixel_vec() should not exist, and this function should
    // support any bitmap format.
    let host_obj = env.objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    assert_eq!(bitmap_data.bits_per_component, 8);
    let raw_bytes = env
        .mem
        .bytes_at(
            bitmap_data.data.cast(),
            bitmap_data.bytes_per_row * bitmap_data.height,
        );
    let pixels = convert_bitmap_context_pixels_to_rgba(&bitmap_data, raw_bytes);
    cg_image::from_image(
        env,
        Image::from_pixel_vec(pixels, (bitmap_data.width, bitmap_data.height)),
    )
}

fn components_for_rgb(bitmap_info: CGBitmapInfo) -> Result<GuestUSize, ()> {
    let byte_order = bitmap_info & kCGBitmapByteOrderMask;
    if byte_order != kCGImageByteOrderDefault
        && byte_order != kCGImageByteOrder32Big
        && byte_order != kCGImageByteOrder32Little
    {
        return Err(()); // TODO: handle other byte orders
    }

    let alpha_info = bitmap_info & kCGBitmapAlphaInfoMask;
    if (alpha_info | byte_order) != bitmap_info {
        return Err(()); // TODO: handle other cases (float)
    }
    match alpha_info & kCGBitmapAlphaInfoMask {
        kCGImageAlphaNone => {
            if byte_order == kCGImageByteOrder32Big || byte_order == kCGImageByteOrder32Little {
                Ok(4)
            } else {
                Ok(3)
            }
        }
        kCGImageAlphaPremultipliedLast
        | kCGImageAlphaPremultipliedFirst
        | kCGImageAlphaLast
        | kCGImageAlphaFirst
        | kCGImageAlphaNoneSkipLast
        | kCGImageAlphaNoneSkipFirst => Ok(4), // RGBA/ARGB/RGBX/XRGB
        kCGImageAlphaOnly => Ok(1), // A
        _ => Err(()),               // unknown values
    }
}

fn components_for_gray(bitmap_info: CGBitmapInfo) -> Result<GuestUSize, ()> {
    let byte_order = bitmap_info & kCGBitmapByteOrderMask;
    if byte_order != kCGImageByteOrderDefault
        && byte_order != kCGImageByteOrder32Big
        && byte_order != kCGImageByteOrder32Little
    {
        return Err(()); // TODO: handle other byte orders
    }

    let alpha_info = bitmap_info & kCGBitmapAlphaInfoMask;
    if (alpha_info | byte_order) != bitmap_info {
        return Err(()); // TODO: handle other cases (float)
    }
    match alpha_info & kCGBitmapAlphaInfoMask {
        kCGImageAlphaNone => Ok(1), // gray
        kCGImageAlphaPremultipliedLast
        | kCGImageAlphaPremultipliedFirst
        | kCGImageAlphaLast
        | kCGImageAlphaFirst
        | kCGImageAlphaNoneSkipLast
        | kCGImageAlphaNoneSkipFirst => Ok(2), // gray + alpha
        kCGImageAlphaOnly => Ok(1), // A
        _ => Err(()),               // unknown values
    }
}

fn bytes_per_pixel(data: &CGBitmapContextData) -> GuestUSize {
    let &CGBitmapContextData {
        bits_per_component,
        color_space,
        alpha_info,
        byte_order,
        width,
        bytes_per_row,
        ..
    } = data;
    assert!(bits_per_component == 8);
    match color_space {
        kCGColorSpaceGenericRGB => {
            if alpha_info == kCGImageAlphaNone
                && (byte_order == kCGImageByteOrder32Little
                    || byte_order == kCGImageByteOrder32Big
                    || (width > 0 && bytes_per_row >= width * 4))
            {
                4
            } else {
                components_for_rgb(alpha_info | byte_order).unwrap()
            }
        }
        kCGColorSpaceGenericGray => components_for_gray(alpha_info | byte_order).unwrap(),
        _ => unimplemented!("support other color spaces"),
    }
}

fn get_pixels<'a>(data: &CGBitmapContextData, mem: &'a mut Mem) -> &'a mut [u8] {
    let pixel_data_size = data.height.checked_mul(data.bytes_per_row).unwrap();
    mem.bytes_at_mut(data.data.cast(), pixel_data_size)
}

/// Blends two RGBA non gamma-encoded values, with straight alpha,
/// using a blend mode.
fn blend_straight(
    bg: (f32, f32, f32, f32),
    fg: (f32, f32, f32, f32),
    blend_mode: CGBlendMode,
) -> (f32, f32, f32, f32) {
    assert_eq!(blend_mode, kCGBlendModeNormal); // TODO
    if fg.3 == 0.0 {
        // If fg.3 == 0.0 we attempt to blend fully transparent color.
        bg
    } else {
        let neg_fg_a = 1.0 - fg.3;
        let new_a = fg.3 + bg.3 * neg_fg_a; // Can't be 0 if fg.3 != 0
        (
            (fg.0 * fg.3 + bg.0 * bg.3 * neg_fg_a) / new_a,
            (fg.1 * fg.3 + bg.1 * bg.3 * neg_fg_a) / new_a,
            (fg.2 * fg.3 + bg.2 * bg.3 * neg_fg_a) / new_a,
            new_a,
        )
    }
}

/// Blends two RGBA non gamma-encoded values, with premultiplied alpha,
/// using a blend mode.
/// See [Blending](https://www.w3.org/TR/compositing-1/#blending) for details.
fn blend_premultiplied(
    bg: (f32, f32, f32, f32),
    fg: (f32, f32, f32, f32),
    blend_mode: CGBlendMode,
) -> (f32, f32, f32, f32) {
    if blend_mode == kCGBlendModeCopy {
        return fg;
    }
    // Blend
    let blend_res = match blend_mode {
        kCGBlendModeNormal => (bg.3 * fg.0, bg.3 * fg.1, bg.3 * fg.2),
        kCGBlendModeMultiply => (bg.0 * fg.0, bg.1 * fg.1, bg.2 * fg.2),
        kCGBlendModeScreen => (
            fg.3 * bg.0 + bg.3 * fg.0 - bg.0 * fg.0,
            fg.3 * bg.1 + bg.3 * fg.1 - bg.1 * fg.1,
            fg.3 * bg.2 + bg.3 * fg.2 - bg.2 * fg.2,
        ),
        kCGBlendModeDarken => (
            (fg.3 * bg.0).min(bg.3 * fg.0),
            (fg.3 * bg.1).min(bg.3 * fg.1),
            (fg.3 * bg.2).min(bg.3 * fg.2),
        ),
        kCGBlendModeLighten => (
            (fg.3 * bg.0).max(bg.3 * fg.0),
            (fg.3 * bg.1).max(bg.3 * fg.1),
            (fg.3 * bg.2).max(bg.3 * fg.2),
        ),
        _ => unimplemented!("blend mode {}", blend_mode),
    };
    // Compose
    let neg_bg_a = 1.0 - bg.3;
    let neg_fg_a = 1.0 - fg.3;
    (
        fg.0 * neg_bg_a + blend_res.0 + bg.0 * neg_fg_a,
        fg.1 * neg_bg_a + blend_res.1 + bg.1 * neg_fg_a,
        fg.2 * neg_bg_a + blend_res.2 + bg.2 * neg_fg_a,
        fg.3 + bg.3 * neg_fg_a,
    )
}

/// per component offsets (r, g, b, a)
fn pixel_offsets(data: &CGBitmapContextData) -> (usize, usize, usize, Option<usize>) {
    match data.color_space {
        kCGColorSpaceGenericRGB => {
            let (r, g, b, a) = match data.alpha_info {
                kCGImageAlphaNone => (0, 1, 2, None),
                kCGImageAlphaPremultipliedLast | kCGImageAlphaLast => (0, 1, 2, Some(3)),
                kCGImageAlphaPremultipliedFirst | kCGImageAlphaFirst => (1, 2, 3, Some(0)),
                kCGImageAlphaNoneSkipLast => (0, 1, 2, None),
                kCGImageAlphaNoneSkipFirst => (1, 2, 3, None),
                kCGImageAlphaOnly => (0, 0, 0, Some(0)),
                _ => unreachable!(), // checked by bytes_per_pixel
            };
            if data.byte_order == kCGImageByteOrder32Little && bytes_per_pixel(data) == 4 {
                (3 - r, 3 - g, 3 - b, a.map(|idx| 3 - idx))
            } else {
                (r, g, b, a)
            }
        }
        kCGColorSpaceGenericGray => {
            // TODO: this is probably isn't doing RGB to grayscale conversion
            // properly
            match data.alpha_info {
                kCGImageAlphaNone => (0, 0, 0, None),
                kCGImageAlphaPremultipliedLast | kCGImageAlphaLast => (0, 0, 0, Some(1)),
                kCGImageAlphaPremultipliedFirst | kCGImageAlphaFirst => (1, 1, 1, Some(0)),
                kCGImageAlphaNoneSkipLast => (0, 0, 0, None),
                kCGImageAlphaNoneSkipFirst => (1, 1, 1, None),
                kCGImageAlphaOnly => (0, 0, 0, Some(0)),
                _ => unreachable!(), // checked by bytes_per_pixel
            }
        }
        _ => unimplemented!(),
    }
}

/// Get gamma-decoded RGBA value.
fn get_pixel(
    data: &CGBitmapContextData,
    pixels: &mut [u8],
    first_component_idx: usize,
) -> (f32, f32, f32, f32) {
    let pixel_offset = pixel_offsets(data);
    let pixel = (
        pixels[first_component_idx + pixel_offset.0] as f32 / 255.0,
        pixels[first_component_idx + pixel_offset.1] as f32 / 255.0,
        pixels[first_component_idx + pixel_offset.2] as f32 / 255.0,
        if let Some(alpha_offest) = pixel_offset.3 {
            pixels[first_component_idx + alpha_offest] as f32 / 255.0
        } else {
            1.0
        },
    );

    (
        gamma_decode(pixel.0),
        gamma_decode(pixel.1),
        gamma_decode(pixel.2),
        pixel.3,
    )
}

fn put_pixel(
    data: &CGBitmapContextData,
    pixels: &mut [u8],
    coords: (i32, i32),
    pixel: (CGFloat, CGFloat, CGFloat, CGFloat),
    blend: bool,
    blend_mode: CGBlendMode,
) {
    let (x, y) = coords;
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as GuestUSize, y as GuestUSize);
    if x >= data.width || y >= data.height {
        return;
    }

    // CG's co-ordinate system puts the origin in the bottom-left corner, but it
    // *seems* like the rows are nonetheless in top-to-bottom order?
    let y = data.height - 1 - y;

    let pixel_size = bytes_per_pixel(data);
    let first_component_idx = (y * data.bytes_per_row + x * pixel_size) as usize;

    let bg_pixel = get_pixel(data, pixels, first_component_idx);

    // Blending like this must be done in linear RGB, so this must come before
    // gamma encoding.
    let (r, g, b, a) = if blend {
        match data.alpha_info {
            kCGImageAlphaLast | kCGImageAlphaFirst => blend_straight(bg_pixel, pixel, blend_mode),
            kCGImageAlphaPremultipliedLast | kCGImageAlphaPremultipliedFirst => {
                blend_premultiplied(bg_pixel, pixel, blend_mode)
            }
            kCGImageAlphaOnly => (
                pixel.0,
                pixel.1,
                pixel.2,
                pixel.3 + bg_pixel.3 * (1.0 - pixel.3),
            ),
            _ => {
                assert_eq!(blend_mode, kCGBlendModeNormal); // TODO
                pixel
            }
        }
    } else {
        pixel
    };

    // Alpha is always linear.
    let (r, g, b) = (gamma_encode(r), gamma_encode(g), gamma_encode(b));
    let pixel_offset = pixel_offsets(data);
    match data.alpha_info {
        kCGImageAlphaOnly => {
            pixels[first_component_idx] = (a * 255.0) as u8;
        }
        _ => {
            pixels[first_component_idx + pixel_offset.0] = (r * 255.0) as u8;
            pixels[first_component_idx + pixel_offset.1] = (g * 255.0) as u8;
            pixels[first_component_idx + pixel_offset.2] = (b * 255.0) as u8;
            if let Some(alpha_offset) = pixel_offset.3 {
                pixels[first_component_idx + alpha_offset] = (a * 255.0) as u8;
            }
        }
    }
}

/// Abstract interface for use by host code that wants to draw in a bitmap
/// context.
pub struct CGBitmapContextDrawer<'a> {
    bitmap_info: CGBitmapContextData,
    rgb_fill_color: (CGFloat, CGFloat, CGFloat, CGFloat),
    blend_mode: CGBlendMode,
    transform: CGAffineTransform,
    pixels: &'a mut [u8],
}
impl CGBitmapContextDrawer<'_> {
    pub fn new<'a>(
        objc: &ObjC,
        mem: &'a mut Mem,
        context: CGContextRef,
    ) -> CGBitmapContextDrawer<'a> {
        let &CGContextHostObject {
            subclass: CGContextSubclass::CGBitmapContext(bitmap_info),
            rgb_fill_color,
            transform,
            blend_mode,
            ..
        } = objc.borrow(context);

        let pixels = get_pixels(&bitmap_info, mem);

        CGBitmapContextDrawer {
            bitmap_info,
            rgb_fill_color,
            blend_mode,
            transform,
            pixels,
        }
    }

    pub fn width(&self) -> GuestUSize {
        self.bitmap_info.width
    }
    pub fn height(&self) -> GuestUSize {
        self.bitmap_info.height
    }
    /// Get the current fill color. The returned color is linear RGB, not sRGB.
    /// It has premultiplied alpha if the context does.
    pub fn rgb_fill_color(&self) -> (CGFloat, CGFloat, CGFloat, CGFloat) {
        let multiply_by = match self.bitmap_info.alpha_info {
            kCGImageAlphaPremultipliedLast | kCGImageAlphaPremultipliedFirst => {
                self.rgb_fill_color.3
            }
            _ => 1.0,
        };
        // Multiplying before decoding matches the Simulator's output.
        (
            gamma_decode(self.rgb_fill_color.0 * multiply_by),
            gamma_decode(self.rgb_fill_color.1 * multiply_by),
            gamma_decode(self.rgb_fill_color.2 * multiply_by),
            self.rgb_fill_color.3, // alpha is always linear
        )
    }
    /// Set the pixel at `coords` to `color`. `color` must be linear RGB, not
    /// sRGB! Note that `coords` are absolute: you must do transformation
    /// yourself.
    pub fn put_pixel(
        &mut self,
        coords: (i32, i32),
        color: (CGFloat, CGFloat, CGFloat, CGFloat),
        blend: bool,
    ) {
        put_pixel(
            &self.bitmap_info,
            self.pixels,
            coords,
            color,
            blend,
            self.blend_mode,
        )
    }

    /// Takes a [CGRect] and applies the current transform to it, and iterates
    /// over the transformed, clipped, absolute integer pixel co-ordinates in
    /// raster order for the target bitmap while providing floating-point
    /// co-ordinates from (0,0) to (1,1) as a reference for sampling e.g. a
    /// texture.
    pub fn iter_transformed_pixels(
        &self,
        untransformed_rect: CGRect,
    ) -> impl Iterator<Item = ((i32, i32), (f32, f32))> {
        let bounding_rect = self.transform.apply_to_rect(untransformed_rect);

        let x_start = bounding_rect.origin.x.round().max(0.0) as GuestUSize;
        let y_start = bounding_rect.origin.y.round().max(0.0) as GuestUSize;
        let x_end = (bounding_rect.origin.x + bounding_rect.size.width)
            .round()
            .min(self.width() as f32) as GuestUSize;
        let y_end = (bounding_rect.origin.y + bounding_rect.size.height)
            .round()
            .min(self.height() as f32) as GuestUSize;

        let inverse_transform = self.transform.invert();

        // TODO: Doing a matrix multiply per-pixel is not optimally efficient.
        // A scanline rasterizer would be better, though we should probably use
        // an existing library for this.
        (y_start..y_end).flat_map(move |y| {
            (x_start..x_end).flat_map(move |x| {
                let untransformed = inverse_transform.apply_to_point(CGPoint {
                    x: x as f32 + 0.5,
                    y: y as f32 + 0.5,
                });
                let x_within =
                    (untransformed.x - untransformed_rect.origin.x) / untransformed_rect.size.width;
                let y_within = (untransformed.y - untransformed_rect.origin.y)
                    / untransformed_rect.size.height;
                if !(0.0..1.0).contains(&x_within) || !(0.0..1.0).contains(&y_within) {
                    None
                } else {
                    Some(((x as i32, y as i32), (x_within, y_within)))
                }
            })
        })
    }
}

#[cfg(test)]
#[test]
fn test_iter_transformed_pixels() {
    use super::CGSize;

    fn make_context(
        width: GuestUSize,
        height: GuestUSize,
        transform: CGAffineTransform,
    ) -> CGBitmapContextDrawer<'static> {
        CGBitmapContextDrawer {
            bitmap_info: CGBitmapContextData {
                data: crate::mem::Ptr::null(),
                data_is_owned: false,
                width,
                height,
                bits_per_component: 8,
                bytes_per_row: 3 * width,
                color_space: "kCGColorSpaceGenericRGB",
                alpha_info: 0,
                byte_order: kCGImageByteOrderDefault,
            },
            rgb_fill_color: (0.0, 0.0, 0.0, 0.0),
            blend_mode: kCGBlendModeNormal,
            transform,
            pixels: &mut [],
        }
    }

    let square_2x2_at_0_0 = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 2.0,
            height: 2.0,
        },
    };
    let square_2x2_at_2_2 = CGRect {
        origin: CGPoint { x: 2.0, y: 2.0 },
        size: CGSize {
            width: 2.0,
            height: 2.0,
        },
    };
    let square_4x4_at_0_0 = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 4.0,
            height: 4.0,
        },
    };

    let upright_square_2x2_at_0_0 = [
        ((0, 0), (0.25, 0.25)),
        ((1, 0), (0.75, 0.25)),
        ((0, 1), (0.25, 0.75)),
        ((1, 1), (0.75, 0.75)),
    ];
    let inverted_square_2x2_at_0_0 = [
        ((0, 0), (0.75, 0.75)),
        ((1, 0), (0.25, 0.75)),
        ((0, 1), (0.75, 0.25)),
        ((1, 1), (0.25, 0.25)),
    ];
    let corner_pixel_of_upright_square_2x2_at_1_1 = [((1, 1), (0.25, 0.25))];

    // Constructed by hand so the results are precise
    let rotation_by_180deg = CGAffineTransform {
        a: -1.0,
        c: 0.0,
        b: 0.0,
        d: -1.0,
        tx: 0.0,
        ty: 0.0,
    };

    assert!(make_context(2, 2, CGAffineTransformIdentity)
        .iter_transformed_pixels(square_2x2_at_0_0)
        .eq(upright_square_2x2_at_0_0.into_iter()));
    assert!(
        make_context(2, 2, CGAffineTransform::make_translation(-2.0, -2.0))
            .iter_transformed_pixels(square_2x2_at_2_2)
            .eq(upright_square_2x2_at_0_0.into_iter())
    );
    assert!(
        make_context(2, 2, CGAffineTransform::make_translation(-1.0, -1.0))
            .iter_transformed_pixels(square_2x2_at_2_2)
            .eq(corner_pixel_of_upright_square_2x2_at_1_1.into_iter())
    );
    assert!(make_context(2, 2, CGAffineTransform::make_scale(0.5, 0.5))
        .iter_transformed_pixels(square_4x4_at_0_0)
        .eq(upright_square_2x2_at_0_0.into_iter()));
    assert!(make_context(2, 2, rotation_by_180deg.translate(-2.0, -2.0))
        .iter_transformed_pixels(square_2x2_at_0_0)
        .eq(inverted_square_2x2_at_0_0.into_iter()));
}

/// Implementation of `CGContextFillRect` (`clear` == [false]) and
/// `CGContextClearRect` (`clear` == [true]) for `CGBitmapContext`.
pub(super) fn fill_rect(env: &mut Environment, context: CGContextRef, rect: CGRect, clear: bool) {
    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);
    let color = if clear {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        drawer.rgb_fill_color()
    };
    // TODO: correct anti-aliasing
    for ((x, y), _) in drawer.iter_transformed_pixels(rect) {
        drawer.put_pixel((x, y), color, /* blend: */ !clear)
    }
}

/// Implementation of `CGContextDrawImage` for `CGBitmapContext`.
pub(super) fn draw_image(
    env: &mut Environment,
    context: CGContextRef,
    rect: CGRect,
    image: CGImageRef,
) {
    let image = cg_image::borrow_image(&env.objc, image);

    let mut drawer = CGBitmapContextDrawer::new(&env.objc, &mut env.mem, context);

    //let _ = std::fs::write(
    //  format!(
    //      "image-{:?}-{:?}.data",
    //      (image as *const _ as *const ()),
    //      image.dimensions()
    //  ),
    //  image.pixels()
    //);

    //let _ = std::fs::write(
    //  format!(
    //      "bitmap-{:?}-{:?}-before.data",
    //      (image as *const _ as *const ()),
    //      (drawer.width(), drawer.height())
    //  ),
    //  &drawer.pixels
    //);

    let (image_width, image_height) = image.dimensions();

    // TODO: non-nearest-neighbour filtering? (what does CG actually do?)

    for ((x, y), (texel_x, texel_y)) in drawer.iter_transformed_pixels(rect) {
        let texel_x = (image_width as f32 * texel_x) as i32;
        // Image is in top-to-bottom order, but the bitmap is bottom-to-top
        let texel_y = (image_height as f32 * (1.0 - texel_y)) as i32;
        // FIXME: might need alpha format conversion here
        if let Some(color) = image.get_pixel((texel_x, texel_y)) {
            drawer.put_pixel((x, y), color, /* blend: */ true)
        }
    }

    //let _ = std::fs::write(
    //  format!(
    //      "bitmap-{:?}-{:?}-after.data",
    //      (image as *const _ as *const ()),
    //      (drawer.width(), drawer.height())
    //  ),
    //  &drawer.pixels
    //);
}

#[allow(rustdoc::broken_intra_doc_links)] // https://github.com/rust-lang/rust/issues/83049
/// Shortcut for [crate::frameworks::core_animation::composition]. This is a
/// workaround for not having a `&mut Environment` that should eventually be
/// removed somehow (TODO).
pub fn get_data(objc: &ObjC, context: CGContextRef) -> (GuestUSize, GuestUSize, MutVoidPtr) {
    let host_obj = objc.borrow::<CGContextHostObject>(context);
    let CGContextSubclass::CGBitmapContext(bitmap_data) = host_obj.subclass;
    (bitmap_data.width, bitmap_data.height, bitmap_data.data)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CGBitmapContextCreate(_, _, _, _, _, _, _)),
    export_c_func!(CGBitmapContextCreateImage(_)),
    export_c_func!(CGBitmapContextGetData(_)),
    export_c_func!(CGBitmapContextGetWidth(_)),
    export_c_func!(CGBitmapContextGetHeight(_)),
    export_c_func!(CGBitmapContextGetBytesPerRow(_)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_context_components_and_bytes_per_pixel() {
        // 0x2001: kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedLast
        let info_32little_premul_last = kCGImageByteOrder32Little | kCGImageAlphaPremultipliedLast;
        assert_eq!(components_for_rgb(info_32little_premul_last), Ok(4));
        assert_eq!(components_for_gray(info_32little_premul_last), Ok(2));

        // 0x2006: kCGBitmapByteOrder32Little | kCGImageAlphaNoneSkipFirst
        let info_32little_skip_first = kCGImageByteOrder32Little | kCGImageAlphaNoneSkipFirst;
        assert_eq!(components_for_rgb(info_32little_skip_first), Ok(4));

        // 0: kCGImageByteOrderDefault | kCGImageAlphaNone
        assert_eq!(components_for_rgb(0), Ok(3));
        assert_eq!(components_for_gray(0), Ok(1));

        // 32-bit Little and Big with AlphaNone must return 4 components (32-bit word)
        let info_32little_alpha_none = kCGImageByteOrder32Little | kCGImageAlphaNone;
        assert_eq!(components_for_rgb(info_32little_alpha_none), Ok(4));
        let info_32big_alpha_none = kCGImageByteOrder32Big | kCGImageAlphaNone;
        assert_eq!(components_for_rgb(info_32big_alpha_none), Ok(4));

        // Alpha only
        assert_eq!(components_for_gray(kCGImageAlphaOnly), Ok(1));

        // Test bytes_per_pixel with 32bpp alpha none (width * 4 bytes per row)
        let data_rgbx = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 10,
            height: 10,
            bits_per_component: 8,
            bytes_per_row: 40,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaNone,
            byte_order: kCGImageByteOrderDefault,
        };
        assert_eq!(bytes_per_pixel(&data_rgbx), 4);

        // Test bytes_per_pixel with 32-little alpha none regardless of bytes_per_row
        let data_32little_none = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 10,
            height: 10,
            bits_per_component: 8,
            bytes_per_row: 40,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaNone,
            byte_order: kCGImageByteOrder32Little,
        };
        assert_eq!(bytes_per_pixel(&data_32little_none), 4);

        // Test bytes_per_pixel with padded rows (bytes_per_row > width * 4)
        let data_rgbx_padded = CGBitmapContextData {
            bytes_per_row: 64, // 10 * 4 = 40, padded to 64
            ..data_rgbx
        };
        assert_eq!(bytes_per_pixel(&data_rgbx_padded), 4);

        // 24bpp RGB
        let data_rgb24 = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 10,
            height: 10,
            bits_per_component: 8,
            bytes_per_row: 30,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaNone,
            byte_order: kCGImageByteOrderDefault,
        };
        assert_eq!(bytes_per_pixel(&data_rgb24), 3);
    }

    #[test]
    fn test_pixel_offsets_for_sepia_32little() {
        // 0x2001: kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedLast
        let data_sepia = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 2,
            height: 2,
            bits_per_component: 8,
            bytes_per_row: 8,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
            byte_order: kCGImageByteOrder32Little,
        };

        let offsets = pixel_offsets(&data_sepia);
        // Offsets in [A, B, G, R] format:
        // Byte 0: A (Some(0))
        // Byte 1: B (offsets.2 = 1)
        // Byte 2: G (offsets.1 = 2)
        // Byte 3: R (offsets.0 = 3)
        assert_eq!(offsets, (3, 2, 1, Some(0)));

        // Big-endian / default byte order should be (0, 1, 2, Some(3))
        let data_big = CGBitmapContextData {
            byte_order: kCGImageByteOrderDefault,
            ..data_sepia
        };
        assert_eq!(pixel_offsets(&data_big), (0, 1, 2, Some(3)));
    }

    #[test]
    fn test_put_get_pixel_32little_channel_order() {
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 1,
            height: 1,
            bits_per_component: 8,
            bytes_per_row: 4,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
            byte_order: kCGImageByteOrder32Little,
        };

        let mut buffer = [0u8; 4];
        // Put a pixel with pure colors: R=1.0, G=0.0, B=0.0, A=1.0
        put_pixel(
            &data,
            &mut buffer,
            (0, 0),
            (1.0, 0.0, 0.0, 1.0),
            false,
            kCGBlendModeNormal,
        );
        // In 32-little premultiplied last: [A, B, G, R]
        // Byte 0: A = 0xFF
        // Byte 1: B = 0x00
        // Byte 2: G = 0x00
        // Byte 3: R = 0xFF
        assert_eq!(buffer, [0xFF, 0x00, 0x00, 0xFF]);

        // Get the pixel back and check RGBA
        let got = get_pixel(&data, &mut buffer, 0);
        assert!((got.0 - 1.0).abs() < 0.01, "R should be 1.0, got {}", got.0);
        assert!(got.1 < 0.01, "G should be 0.0, got {}", got.1);
        assert!(got.2 < 0.01, "B should be 0.0, got {}", got.2);
        assert!((got.3 - 1.0).abs() < 0.01, "A should be 1.0, got {}", got.3);
    }

    #[test]
    fn test_sepia_loop_and_image_conversion_end_to_end() {
        // Simulate the exact MapleStory death crash sequence:
        // 1. CGBitmapContext with bitmap_info = 0x2001 (kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedLast)
        let data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 2,
            height: 1,
            bits_per_component: 8,
            bytes_per_row: 8,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaPremultipliedLast,
            byte_order: kCGImageByteOrder32Little,
        };

        let mut buffer = vec![0u8; 8];

        // 2. CGContextDrawImage writes pixels into the context (calling put_pixel)
        // Pixel 0: linear color (0.8, 0.6, 0.4, 1.0)
        put_pixel(
            &data,
            &mut buffer,
            (0, 0),
            (0.8, 0.6, 0.4, 1.0),
            false,
            kCGBlendModeNormal,
        );
        // Pixel 1: linear color (0.2, 0.4, 0.6, 0.5)
        put_pixel(
            &data,
            &mut buffer,
            (1, 0),
            (0.2, 0.4, 0.6, 0.5),
            false,
            kCGBlendModeNormal,
        );

        // Verify buffer has [A, B, G, R] for each pixel:
        // Byte 0 is A, Byte 1 is B, Byte 2 is G, Byte 3 is R
        let p0_a = buffer[0];
        let p0_b = buffer[1];
        let p0_g = buffer[2];
        let p0_r = buffer[3];
        assert_eq!(p0_a, 255); // Alpha is 1.0
        assert!(p0_b > 0 && p0_g > 0 && p0_r > 0);

        // 3. +[MDGame convertToSepia:] executes the guest ARM loop:
        // It skips byte 0 (alpha) and modifies byte 1 (B), byte 2 (G), byte 3 (R):
        for chunk in buffer.chunks_exact_mut(4) {
            let b = chunk[1] as f32;
            let g = chunk[2] as f32;
            let r = chunk[3] as f32;
            // Sepia transformation formula:
            let sepia_r = (r * 0.393 + g * 0.769 + b * 0.189).min(255.0) as u8;
            let sepia_g = (r * 0.349 + g * 0.686 + b * 0.168).min(255.0) as u8;
            let sepia_b = (r * 0.272 + g * 0.534 + b * 0.131).min(255.0) as u8;
            chunk[1] = sepia_b;
            chunk[2] = sepia_g;
            chunk[3] = sepia_r;
        }

        // 4. CGBitmapContextCreateImage converts the [A, B, G, R] buffer back to [R, G, B, A]
        let rgba_pixels = convert_bitmap_context_pixels_to_rgba(&data, &buffer);
        assert_eq!(rgba_pixels.len(), 8);

        // Pixel 0 check:
        // rgba_pixels[0] is R (from chunk[3])
        // rgba_pixels[1] is G (from chunk[2])
        // rgba_pixels[2] is B (from chunk[1])
        // rgba_pixels[3] is A (from chunk[0])
        assert_eq!(rgba_pixels[0], buffer[3]);
        assert_eq!(rgba_pixels[1], buffer[2]);
        assert_eq!(rgba_pixels[2], buffer[1]);
        assert_eq!(rgba_pixels[3], buffer[0]);

        // Pixel 1 check:
        assert_eq!(rgba_pixels[4], buffer[7]);
        assert_eq!(rgba_pixels[5], buffer[6]);
        assert_eq!(rgba_pixels[6], buffer[5]);
        assert_eq!(rgba_pixels[7], buffer[4]);
    }

    #[test]
    fn test_convert_bitmap_context_pixels_gray_and_alpha_only() {
        let gray_data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 2,
            height: 1,
            bits_per_component: 8,
            bytes_per_row: 2,
            color_space: kCGColorSpaceGenericGray,
            alpha_info: kCGImageAlphaNone,
            byte_order: kCGImageByteOrderDefault,
        };
        let gray_buf = [128u8, 200u8];
        let gray_rgba = convert_bitmap_context_pixels_to_rgba(&gray_data, &gray_buf);
        assert_eq!(
            gray_rgba,
            vec![128, 128, 128, 0xFF, 200, 200, 200, 0xFF]
        );

        let alpha_only_data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 2,
            height: 1,
            bits_per_component: 8,
            bytes_per_row: 2,
            color_space: kCGColorSpaceGenericGray,
            alpha_info: kCGImageAlphaOnly,
            byte_order: kCGImageByteOrderDefault,
        };
        let alpha_buf = [0x55u8, 0xAAu8];
        let alpha_rgba = convert_bitmap_context_pixels_to_rgba(&alpha_only_data, &alpha_buf);
        assert_eq!(
            alpha_rgba,
            vec![0, 0, 0, 0x55, 0, 0, 0, 0xAA]
        );
    }

    #[test]
    fn test_convert_bitmap_context_pixels_straight_alpha_premultiplied() {
        // Straight alpha (kCGImageAlphaLast): 50% opacity (128) should halve the RGB values.
        let straight_alpha_data = CGBitmapContextData {
            data: Ptr::null(),
            data_is_owned: false,
            width: 1,
            height: 1,
            bits_per_component: 8,
            bytes_per_row: 4,
            color_space: kCGColorSpaceGenericRGB,
            alpha_info: kCGImageAlphaLast,
            byte_order: kCGImageByteOrderDefault,
        };
        let buf = [200u8, 100u8, 50u8, 128u8]; // [R, G, B, A] straight
        let rgba = convert_bitmap_context_pixels_to_rgba(&straight_alpha_data, &buf);
        // Premultiplied:
        // R: 200 * (128 / 255) = 100
        // G: 100 * (128 / 255) = 50
        // B: 50 * (128 / 255) = 25
        // A: 128
        assert_eq!(rgba[3], 128);
        assert!((rgba[0] as i32 - 100).abs() <= 1);
        assert!((rgba[1] as i32 - 50).abs() <= 1);
        assert!((rgba[2] as i32 - 25).abs() <= 1);
    }
}

