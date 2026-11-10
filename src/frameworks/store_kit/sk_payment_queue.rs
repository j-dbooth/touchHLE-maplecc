/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::objc::{
    id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr,
};

struct SKPaymentQueueHostObject {
    observers: Vec<id>,
}
impl HostObject for SKPaymentQueueHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SKPaymentQueue: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SKPaymentQueueHostObject {
        observers: Vec::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)defaultQueue {
    static mut QUEUE: id = nil;
    unsafe {
        if QUEUE == nil {
            let q: id = msg![env; this alloc];
            let q: id = msg![env; q init];
            QUEUE = q;
        }
        QUEUE
    }
}

+ (bool)canMakePayments {
    true
}

- (())addTransactionObserver:(id)observer {
    log_dbg!("SKPaymentQueue addTransactionObserver:{:?}", observer);
    let host = env.objc.borrow_mut::<SKPaymentQueueHostObject>(this);
    if !host.observers.contains(&observer) {
        host.observers.push(observer);
    }
}

- (())removeTransactionObserver:(id)observer {
    log_dbg!("SKPaymentQueue removeTransactionObserver:{:?}", observer);
    let host = env.objc.borrow_mut::<SKPaymentQueueHostObject>(this);
    host.observers.retain(|&o| o != observer);
}

- (())addPayment:(id)payment {
    log_dbg!("SKPaymentQueue addPayment:{:?}", payment);
    if payment == nil {
        return;
    }
    let sel = env.objc.register_host_selector("_processPayment:".to_string(), &mut env.mem);
    let _: () = msg![env; this performSelector:sel withObject:payment afterDelay:0.05f64];
}

- (())_processPayment:(id)payment {
    log_dbg!("SKPaymentQueue _processPayment:{:?}", payment);
    if payment == nil {
        return;
    }

    let ident_id: id = msg![env; payment productIdentifier];
    let ident = if ident_id != nil {
        crate::frameworks::foundation::ns_string::to_rust_string(env, ident_id).into_owned()
    } else {
        String::new()
    };
    let price = super::sk_product::get_item_price_in_meso(&ident);

    let md_player_cls = env.objc.find_class("MDPlayer", &mut env.mem);
    let success = if let Some(cls) = md_player_cls {
        let player: id = msg![env; cls sharedMDPlayer];
        if player != nil {
            let current_gold: i32 = msg![env; player gold];
            if current_gold >= price as i32 {
                let new_gold = current_gold - price as i32;
                let _: () = msg![env; player setGold:new_gold];
                log_dbg!("Cash Shop: Purchased {} for {} Meso. (Meso: {} -> {})", ident, price, current_gold, new_gold);
                true
            } else {
                log_dbg!("Cash Shop: Purchase failed! Need {} Meso, but player only has {} Meso.", price, current_gold);
                false
            }
        } else {
            true
        }
    } else {
        true
    };

    let (state, err) = if success {
        (1, nil)
    } else {
        let err: id = msg_class![env; NSError alloc];
        let domain = crate::frameworks::foundation::ns_string::from_rust_string(env, "SKErrorDomain".to_string());
        let err: id = msg![env; err initWithDomain:domain code:1 userInfo:nil];
        (2, err)
    };

    let transaction: id = msg_class![env; SKPaymentTransaction alloc];
    let transaction: id = msg![env; transaction init];
    retain(env, payment);
    let trans_host = env.objc.borrow_mut::<super::sk_product::SKPaymentTransactionHostObject>(transaction);
    trans_host.payment = payment;
    trans_host.transaction_state = state;
    trans_host.error = err;

    let observers = env.objc.borrow::<SKPaymentQueueHostObject>(this).observers.clone();
    let trans_array: id = msg_class![env; NSArray arrayWithObject:transaction];
    for observer in observers {
        let did_update_sel = env.objc.register_host_selector("paymentQueue:updatedTransactions:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, observer, did_update_sel) {
            let _: () = crate::objc::msg_send(env, (observer, did_update_sel, this, trans_array));
        }

        // In case connectionNotice overlay is still active, dismiss it
        let notice_sel = env.objc.register_host_selector("connectionNotice".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, observer, notice_sel) {
            let notice: id = msg![env; observer connectionNotice];
            if notice != nil {
                let close_sel = env.objc.register_host_selector("closeWithButtonIndex:".to_string(), &mut env.mem);
                if env.objc.object_has_method(&env.mem, notice, close_sel) {
                    let _: () = msg![env; notice closeWithButtonIndex:0];
                }
            }
        }
    }
    release(env, transaction);
}

- (())restoreCompletedTransactions {
    log_dbg!("SKPaymentQueue restoreCompletedTransactions");
    let observers = env.objc.borrow::<SKPaymentQueueHostObject>(this).observers.clone();
    for observer in observers {
        let did_finish_sel = env.objc.register_host_selector("paymentQueueRestoreCompletedTransactionsFinished:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, observer, did_finish_sel) {
            let _: () = crate::objc::msg_send(env, (observer, did_finish_sel, this));
        }
    }
}

- (())finishTransaction:(id)transaction {
    log_dbg!("SKPaymentQueue finishTransaction:{:?}", transaction);
}

- (id)transactions {
    msg_class![env; NSArray array]
}

@end

};
