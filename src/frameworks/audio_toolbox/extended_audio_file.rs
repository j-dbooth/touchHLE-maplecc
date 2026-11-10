/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ExtendedAudioFile.h` (Extended Audio File Services)
//!
//! Currently implemented as a dummy wrapper around Audio File Services.

// TODO: Audio format conversion

use super::audio_file::{
    kAudioFileBadPropertySizeError, kAudioFilePropertyDataFormat, kAudioFileReadPermission,
    kAudioFileUnsupportedFileTypeError, kAudioFileUnsupportedPropertyError,
    kAudioFileUnspecifiedError, AudioFileClose, AudioFileGetProperty, AudioFileID, AudioFileOpenURL,
    AudioFileReadBytes,
};
use super::audio_queue::is_supported_audio_format;
use super::audio_unit::AudioBufferList;
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::carbon_core::{eofErr, OSStatus};
use crate::frameworks::core_audio_types::{
    debug_fourcc, fourcc, kAudioFormatLinearPCM, AudioStreamBasicDescription,
};
use crate::frameworks::core_foundation::cf_url::CFURLRef;
use crate::frameworks::foundation::ns_url::to_rust_path;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub extended_audio_files: HashMap<ExtAudioFileRef, ExtAudioFileHostObject>,
}
impl State {
    pub fn get(framework_state: &mut crate::frameworks::State) -> &mut Self {
        &mut framework_state.audio_toolbox.extended_audio_file
    }
}

pub struct ExtAudioFileHostObject {
    guest_audio_file: AudioFileID,
    client_data_format: Option<AudioStreamBasicDescription>,
    current_bytes_read: i64,
}

#[repr(C, packed)]
pub struct OpaqueExtAudioFile {
    _filler: u8,
}
unsafe impl SafeRead for OpaqueExtAudioFile {}

type ExtAudioFileRef = MutPtr<OpaqueExtAudioFile>;

/// Usually a FourCC.
type ExtAudioFilePropertyID = u32;
const kExtAudioFileProperty_FileDataFormat: ExtAudioFilePropertyID = fourcc(b"ffmt");
const kExtAudioFileProperty_FileLengthFrames: ExtAudioFilePropertyID = fourcc(b"#frm");
const kExtAudioFileProperty_ClientDataFormat: ExtAudioFilePropertyID = fourcc(b"cfmt");

fn ExtAudioFileOpenURL(
    env: &mut Environment,
    in_url: CFURLRef,
    out_ext_audio_file: MutPtr<ExtAudioFileRef>,
) -> OSStatus {
    return_if_null!(in_url);

    let path = to_rust_path(env, in_url);
    log_dbg!(
        "ExtAudioFileOpenURL({:?} '{:?}', {:?})",
        in_url,
        path,
        out_ext_audio_file
    );

    let audio_file_ptr: MutPtr<AudioFileID> = env.mem.alloc(guest_size_of::<AudioFileID>()).cast();
    let res = AudioFileOpenURL(env, in_url, kAudioFileReadPermission, 0, audio_file_ptr);
    let guest_audio_file = env.mem.read(audio_file_ptr);
    env.mem.free(audio_file_ptr.cast());
    if res != 0 {
        log!(
            "ExtAudioFileOpenURL({:?} '{:?}', {:?}) failed, error: {:?}",
            in_url,
            path,
            out_ext_audio_file,
            res
        );
        return res;
    }

    let host_object = ExtAudioFileHostObject {
        guest_audio_file,
        client_data_format: None,
        current_bytes_read: 0,
    };

    let guest_extended_audio_file = env.mem.alloc_and_write(OpaqueExtAudioFile { _filler: 0 });
    State::get(&mut env.framework_state)
        .extended_audio_files
        .insert(guest_extended_audio_file, host_object);

    env.mem.write(out_ext_audio_file, guest_extended_audio_file);

    0 // success
}

fn ExtAudioFileGetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    io_property_data_size: MutPtr<u32>,
    out_property_data: MutVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);
    return_if_null!(io_property_data_size);

    let (guest_audio_file, client_data_format) = {
        let host_object = match env
            .framework_state
            .audio_toolbox
            .extended_audio_file
            .extended_audio_files
            .get(&in_ext_audio_file)
        {
            Some(obj) => obj,
            None => return kAudioFileUnspecifiedError,
        };
        (host_object.guest_audio_file, host_object.client_data_format)
    };

    match in_property_id {
        kExtAudioFileProperty_FileDataFormat => {
            let required_size = guest_size_of::<AudioStreamBasicDescription>();
            if env.mem.read(io_property_data_size) < required_size {
                log!("Warning: ExtAudioFileGetProperty(kExtAudioFileProperty_FileDataFormat) failed: bad property size");
                return kAudioFileBadPropertySizeError;
            }
            env.mem.write(io_property_data_size, required_size);
            AudioFileGetProperty(
                env,
                guest_audio_file,
                kAudioFilePropertyDataFormat,
                io_property_data_size,
                out_property_data,
            )
        }
        kExtAudioFileProperty_FileLengthFrames => {
            let required_size = guest_size_of::<i64>();
            if env.mem.read(io_property_data_size) < required_size {
                log!("Warning: ExtAudioFileGetProperty(kExtAudioFileProperty_FileLengthFrames) failed: bad property size");
                return kAudioFileBadPropertySizeError;
            }
            let audio_file_host_object = match env
                .framework_state
                .audio_toolbox
                .audio_file
                .audio_files
                .get(&guest_audio_file)
            {
                Some(obj) => obj,
                None => return kAudioFileUnspecifiedError,
            };
            let desc = audio_file_host_object.audio_file.audio_description();
            let frames_per_packet = (desc.frames_per_packet as u64).max(1);
            let packet_count = audio_file_host_object.audio_file.packet_count();
            let total_frames = (packet_count * frames_per_packet) as i64;
            if !out_property_data.is_null() {
                env.mem.write(out_property_data.cast(), total_frames);
            }
            env.mem.write(io_property_data_size, required_size);
            0 // success
        }
        kExtAudioFileProperty_ClientDataFormat => {
            let required_size = guest_size_of::<AudioStreamBasicDescription>();
            if env.mem.read(io_property_data_size) < required_size {
                log!("Warning: ExtAudioFileGetProperty(kExtAudioFileProperty_ClientDataFormat) failed: bad property size");
                return kAudioFileBadPropertySizeError;
            }
            if let Some(client_desc) = client_data_format {
                if !out_property_data.is_null() {
                    env.mem.write(out_property_data.cast(), client_desc);
                }
                env.mem.write(io_property_data_size, required_size);
                0 // success
            } else {
                env.mem.write(io_property_data_size, required_size);
                AudioFileGetProperty(
                    env,
                    guest_audio_file,
                    kAudioFilePropertyDataFormat,
                    io_property_data_size,
                    out_property_data,
                )
            }
        }
        _ => {
            log!(
                "Warning: ExtAudioFileGetProperty() unimplemented property ID: {}",
                debug_fourcc(in_property_id)
            );
            kAudioFileUnsupportedPropertyError
        }
    }
}

fn ExtAudioFileSetProperty(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    in_property_id: ExtAudioFilePropertyID,
    in_property_data_size: u32,
    in_property_data: ConstVoidPtr,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);
    return_if_null!(in_property_data);

    if in_property_id != kExtAudioFileProperty_ClientDataFormat {
        log!(
            "Warning: ExtAudioFileSetProperty() unimplemented property ID: {}",
            debug_fourcc(in_property_id)
        );
        return kAudioFileUnsupportedPropertyError;
    }

    if in_property_data_size != guest_size_of::<AudioStreamBasicDescription>() {
        log!("Warning: ExtAudioFileSetProperty(ClientDataFormat) bad size");
        return kAudioFileBadPropertySizeError;
    }

    let audio_desc_ptr: ConstPtr<AudioStreamBasicDescription> = in_property_data.cast();
    let client_audio_desc = env.mem.read(audio_desc_ptr);
    log_dbg!("ExtAudioFileSetProperty {:?}", client_audio_desc);
    let format_id = client_audio_desc.format_id;
    if format_id != kAudioFormatLinearPCM || !is_supported_audio_format(&client_audio_desc) {
        log!(
            "Warning: ExtAudioFileSetProperty unsupported audio format {:?}",
            client_audio_desc
        );
        return kAudioFileUnsupportedFileTypeError;
    }

    let (guest_audio_file, had_client_format) = {
        let host_object = match env
            .framework_state
            .audio_toolbox
            .extended_audio_file
            .extended_audio_files
            .get_mut(&in_ext_audio_file)
        {
            Some(obj) => {
                let had_format = obj.client_data_format.is_some();
                obj.client_data_format = Some(client_audio_desc);
                (obj.guest_audio_file, had_format)
            }
            None => return kAudioFileUnspecifiedError,
        };
        host_object
    };

    if had_client_format {
        log!("Warning: ExtAudioFileSetProperty client data format was already set, overwriting");
    }

    let other_host_object = env
        .framework_state
        .audio_toolbox
        .audio_file
        .audio_files
        .get(&guest_audio_file)
        .unwrap();
    let audio_desc = AudioStreamBasicDescription::from_audio_description(
        other_host_object.audio_file.audio_description(),
    );
    // TODO: support audio format conversions
    if audio_desc != client_audio_desc {
        log!(
            "Warning: ExtAudioFileSetProperty client format {:?} does not match file format {:?}, format conversion is not yet fully supported",
            client_audio_desc,
            audio_desc
        );
    }

    0 // success
}

fn ExtAudioFileRead(
    env: &mut Environment,
    in_ext_audio_file: ExtAudioFileRef,
    io_number_frames: MutPtr<u32>,
    io_data: MutPtr<AudioBufferList<1>>,
) -> OSStatus {
    return_if_null!(in_ext_audio_file);
    return_if_null!(io_number_frames);
    return_if_null!(io_data);

    let mut audio_buffer_list = env.mem.read(io_data);
    let num_buffers = audio_buffer_list.number_buffers;
    if num_buffers != 1 {
        log!(
            "Warning: ExtAudioFileRead() unsupported number of buffers: {}",
            num_buffers
        );
        return kAudioFileUnspecifiedError;
    }

    let host_object = match env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get(&in_ext_audio_file)
    {
        Some(obj) => obj,
        None => return kAudioFileUnspecifiedError,
    };

    let client_data_format = match host_object.client_data_format {
        Some(desc) => desc,
        None => {
            let other_host_object = env
                .framework_state
                .audio_toolbox
                .audio_file
                .audio_files
                .get(&host_object.guest_audio_file)
                .unwrap();
            AudioStreamBasicDescription::from_audio_description(
                other_host_object.audio_file.audio_description(),
            )
        }
    };

    audio_buffer_list.buffers[0].number_channels = client_data_format.channels_per_frame;

    let number_frames = env.mem.read(io_number_frames);
    if number_frames == 0 {
        return 0;
    }
    let bytes_per_frame = client_data_format.bytes_per_frame;
    let number_of_bytes = match number_frames.checked_mul(bytes_per_frame) {
        Some(b) => b,
        None => return kAudioFileUnspecifiedError,
    };
    let number_of_bytes_ptr = env.mem.alloc_and_write(number_of_bytes);
    let res = AudioFileReadBytes(
        env,
        host_object.guest_audio_file,
        false,
        host_object.current_bytes_read,
        number_of_bytes_ptr,
        audio_buffer_list.buffers[0].data,
    );
    let number_of_bytes_read = env.mem.read(number_of_bytes_ptr);
    env.mem.free(number_of_bytes_ptr.cast());
    if res != 0 && res != eofErr {
        log!(
            "ExtAudioFileRead({:?}, {:?}, {:?}) failed, error: {:?}",
            in_ext_audio_file,
            io_number_frames,
            io_data,
            res
        );
        return res;
    }
    let frames_read = if bytes_per_frame > 0 {
        number_of_bytes_read / bytes_per_frame
    } else {
        0
    };
    env.mem.write(io_number_frames, frames_read);
    audio_buffer_list.buffers[0].data_byte_size = number_of_bytes_read;

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .get_mut(&in_ext_audio_file)
        .unwrap();
    host_object.current_bytes_read += number_of_bytes_read as i64;

    env.mem.write(io_data, audio_buffer_list);

    0 // success
}

fn ExtAudioFileDispose(env: &mut Environment, in_ext_audio_file: ExtAudioFileRef) -> OSStatus {
    return_if_null!(in_ext_audio_file);

    let host_object = env
        .framework_state
        .audio_toolbox
        .extended_audio_file
        .extended_audio_files
        .remove(&in_ext_audio_file);

    let Some(host_object) = host_object else {
        return kAudioFileUnspecifiedError;
    };

    let res = AudioFileClose(env, host_object.guest_audio_file);
    if res != 0 {
        log!(
            "Warning: ExtAudioFileDispose: AudioFileClose failed with error: {}",
            res
        );
    }

    env.mem.free(in_ext_audio_file.cast());

    0 // success
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(ExtAudioFileOpenURL(_, _)),
    export_c_func!(ExtAudioFileGetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileSetProperty(_, _, _, _)),
    export_c_func!(ExtAudioFileRead(_, _, _)),
    export_c_func!(ExtAudioFileDispose(_)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_constants() {
        assert_eq!(kExtAudioFileProperty_FileDataFormat, 0x66666d74);
        assert_eq!(kExtAudioFileProperty_FileLengthFrames, 0x2366726d);
        assert_eq!(kExtAudioFileProperty_ClientDataFormat, 0x63666d74);
    }

    #[test]
    fn test_frame_calculation() {
        let packet_count: u64 = 44100;
        let frames_per_packet: u64 = 1;
        let total_frames = (packet_count * frames_per_packet) as i64;
        assert_eq!(total_frames, 44100i64);

        let large_packet_count: u64 = 192000 * 3600; // 1 hour at 192kHz
        let total_large_frames = (large_packet_count * frames_per_packet) as i64;
        assert_eq!(total_large_frames, 691200000i64);
    }

    #[test]
    fn test_partial_buffer_frame_calculation() {
        let bytes_per_frame = 4; // 16-bit stereo
        let requested_frames = 1024;
        let requested_bytes = requested_frames * bytes_per_frame;
        assert_eq!(requested_bytes, 4096);

        // Simulate reaching EOF with partial data (e.g. 1500 bytes read out of 4096)
        let bytes_read = 1500;
        let frames_read = bytes_read / bytes_per_frame;
        assert_eq!(frames_read, 375);

        // When at EOF already (0 bytes read)
        let eof_bytes_read = 0;
        let eof_frames_read = eof_bytes_read / bytes_per_frame;
        assert_eq!(eof_frames_read, 0);
    }
}

