/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `GKLocalPlayer`.

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::ns_string::from_rust_string;
use crate::mem::ConstPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, retain, ClassExports, HostObject,
    NSZonePtr,
};

#[derive(Default)]
pub struct GKLocalPlayerHostObject {
    authenticated: bool,
}
impl HostObject for GKLocalPlayerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

// TODO: proper inheritance chain
@implementation GKLocalPlayer: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<GKLocalPlayerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)localPlayer {
    let existing = env.framework_state.game_kit.local_player;
    if existing != nil {
        return existing;
    }
    let player: id = msg_class![env; GKLocalPlayer alloc];
    let player: id = msg![env; player init];
    retain(env, player);
    env.framework_state.game_kit.local_player = player;
    player
}

- (id)init {
    let this: id = crate::objc::msg_super![env; this init];
    env.objc.borrow_mut::<GKLocalPlayerHostObject>(this).authenticated = true;
    this
}

- (bool)isAuthenticated {
    env.objc.borrow::<GKLocalPlayerHostObject>(this).authenticated
}

- (id)alias {
    let s = from_rust_string(env, "Player".to_string());
    autorelease(env, s)
}

- (id)playerID {
    let s = from_rust_string(env, "G:10000000".to_string());
    autorelease(env, s)
}

- (())authenticateWithCompletionHandler:(id)block {
    log_dbg!("GKLocalPlayer authenticateWithCompletionHandler:{:?}", block);
    env.objc.borrow_mut::<GKLocalPlayerHostObject>(this).authenticated = true;
    if block != nil {
        let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
        let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let (): () = invoke.call_from_host(env, (block, nil));
    }
}

@end

};

pub const GKPlayerAuthenticationDidChangeNotificationName: &str =
    "GKPlayerAuthenticationDidChangeNotificationName";

/// `NSNotificationName` values.
pub const CONSTANTS: ConstantExports = &[(
    "_GKPlayerAuthenticationDidChangeNotificationName",
    HostConstant::NSString(GKPlayerAuthenticationDidChangeNotificationName),
)];
