/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `GKAchievement`, `GKAchievementDescription`, and `GKAchievementViewController`.

use crate::abi::{CallFromHost, GuestFunction};
use crate::frameworks::foundation::NSInteger;
use crate::mem::ConstPtr;
use crate::objc::{
    id, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};

struct GKAchievementHostObject {
    identifier: id,
    percent_complete: f64,
    completed: bool,
    shows_completion_banner: bool,
}
impl HostObject for GKAchievementHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation GKAchievement: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(GKAchievementHostObject {
        identifier: nil,
        percent_complete: 0.0,
        completed: false,
        shows_completion_banner: false,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (())loadAchievementsWithCompletionHandler:(id)block {
    if block != nil {
        let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
        let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let empty_arr: id = msg_class![env; NSArray array];
        let (): () = invoke.call_from_host(env, (block, empty_arr, nil));
    }
}

+ (())resetAchievementsWithCompletionHandler:(id)block {
    if block != nil {
        let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
        let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let (): () = invoke.call_from_host(env, (block, nil));
    }
}

- (id)initWithIdentifier:(id)identifier {
    retain(env, identifier);
    env.objc.borrow_mut::<GKAchievementHostObject>(this).identifier = identifier;
    this
}

- (id)identifier {
    env.objc.borrow::<GKAchievementHostObject>(this).identifier
}

- (())setIdentifier:(id)identifier {
    retain(env, identifier);
    let old = std::mem::replace(
        &mut env.objc.borrow_mut::<GKAchievementHostObject>(this).identifier,
        identifier,
    );
    release(env, old);
}

- (f64)percentComplete {
    env.objc.borrow::<GKAchievementHostObject>(this).percent_complete
}

- (())setPercentComplete:(f64)percent {
    env.objc.borrow_mut::<GKAchievementHostObject>(this).percent_complete = percent;
    if percent >= 100.0 {
        env.objc.borrow_mut::<GKAchievementHostObject>(this).completed = true;
    }
}

- (bool)isCompleted {
    env.objc.borrow::<GKAchievementHostObject>(this).completed
}

- (())setCompleted:(bool)completed {
    env.objc.borrow_mut::<GKAchievementHostObject>(this).completed = completed;
}

- (bool)showsCompletionBanner {
    env.objc.borrow::<GKAchievementHostObject>(this).shows_completion_banner
}

- (())setShowsCompletionBanner:(bool)shows {
    env.objc.borrow_mut::<GKAchievementHostObject>(this).shows_completion_banner = shows;
}

- (id)lastReportedDate {
    msg_class![env; NSDate date]
}

- (())reportAchievementWithCompletionHandler:(id)block {
    if block != nil {
        let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
        let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let (): () = invoke.call_from_host(env, (block, nil));
    }
}

- (())dealloc {
    let identifier = env.objc.borrow::<GKAchievementHostObject>(this).identifier;
    release(env, identifier);
    env.objc.dealloc_object(this, &mut env.mem);
}

@end

@implementation GKAchievementDescription: NSObject

+ (())loadAchievementDescriptionsWithCompletionHandler:(id)block {
    if block != nil {
        let invoke_addr: u32 = env.mem.read(ConstPtr::<u32>::from_bits(block.to_bits() + 12));
        let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let empty_arr: id = msg_class![env; NSArray array];
        let (): () = invoke.call_from_host(env, (block, empty_arr, nil));
    }
}

- (id)identifier { nil }
- (id)title { nil }
- (id)achievedDescription { nil }
- (id)unachievedDescription { nil }
- (NSInteger)maximumPoints { 100 }
- (bool)isHidden { false }

@end

@implementation GKAchievementViewController: UIViewController

- (id)achievementDelegate { nil }
- (())setAchievementDelegate:(id)_delegate {}

@end

};
