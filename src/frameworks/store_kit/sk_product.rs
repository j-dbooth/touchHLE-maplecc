/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::frameworks::foundation::ns_array;
use crate::frameworks::foundation::ns_string::{from_rust_string, to_rust_string};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};

pub fn get_item_price_in_meso(identifier: &str) -> u32 {
    if identifier.ends_with("SuperPotion") {
        10_000
    } else if identifier.ends_with("StairScroll") {
        2_000
    } else if identifier.ends_with("Hourglass") {
        2_000
    } else if identifier.ends_with("GoddessBless") {
        10_000
    } else if identifier.ends_with("EnchantWeaponScrollLv4") {
        150_000
    } else if identifier.ends_with("EnchantWeaponScrollLv5") {
        200_000
    } else if identifier.ends_with("EnchantArmorScrollLv4") {
        150_000
    } else if identifier.ends_with("EnchantArmorScrollLv5") {
        200_000
    } else if identifier.ends_with("RestoreWeaponScroll") {
        5_000
    } else if identifier.ends_with("RestoreArmorScroll") {
        5_000
    } else if identifier.ends_with("GoldIngot1") {
        5_000
    } else if identifier.ends_with("GoldIngot2") {
        30_000
    } else if identifier.ends_with("GoldIngot3") {
        80_000
    } else if identifier.ends_with("GoldIngot4") {
        450_000
    } else if identifier.ends_with("GoldIngot5") {
        999_999
    } else {
        10_000
    }
}

pub struct SKProductHostObject {
    pub product_identifier: id,
    pub price: id,
    pub localized_title: id,
    pub localized_description: id,
    pub price_locale: id,
}
impl HostObject for SKProductHostObject {}

#[derive(Default)]
pub struct SKProductsRequestHostObject {
    pub delegate: id,
    pub product_identifiers: id,
}
impl HostObject for SKProductsRequestHostObject {}

pub struct SKProductsResponseHostObject {
    pub products: id,
    pub invalid_product_identifiers: id,
}
impl HostObject for SKProductsResponseHostObject {}

pub struct SKPaymentHostObject {
    pub product_identifier: id,
    pub quantity: NSInteger,
}
impl HostObject for SKPaymentHostObject {}

pub struct SKPaymentTransactionHostObject {
    pub payment: id,
    pub transaction_state: NSInteger,
    pub transaction_identifier: id,
    pub transaction_date: id,
    pub error: id,
}
impl HostObject for SKPaymentTransactionHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation SKProduct: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SKProductHostObject {
        product_identifier: nil,
        price: nil,
        localized_title: nil,
        localized_description: nil,
        price_locale: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)productIdentifier {
    env.objc.borrow::<SKProductHostObject>(this).product_identifier
}

- (id)localizedTitle {
    let title = env.objc.borrow::<SKProductHostObject>(this).localized_title;
    if title != nil {
        title
    } else {
        from_rust_string(env, "".to_string())
    }
}

- (id)localizedDescription {
    let desc = env.objc.borrow::<SKProductHostObject>(this).localized_description;
    if desc != nil {
        desc
    } else {
        from_rust_string(env, "".to_string())
    }
}

- (id)price {
    let price = env.objc.borrow::<SKProductHostObject>(this).price;
    if price != nil {
        price
    } else {
        msg_class![env; NSDecimalNumber zero]
    }
}

- (id)priceLocale {
    let loc = env.objc.borrow::<SKProductHostObject>(this).price_locale;
    if loc != nil {
        loc
    } else {
        msg_class![env; NSLocale currentLocale]
    }
}

- (())dealloc {
    let (ident, price, title, desc, loc) = {
        let host = env.objc.borrow::<SKProductHostObject>(this);
        (host.product_identifier, host.price, host.localized_title, host.localized_description, host.price_locale)
    };
    release(env, ident);
    release(env, price);
    release(env, title);
    release(env, desc);
    release(env, loc);
    crate::objc::msg_super![env; this dealloc]
}

@end

@implementation SKProductsRequest: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<SKProductsRequestHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithProductIdentifiers:(id)ids { // NSSet *
    let this: id = crate::objc::msg_super![env; this init];
    retain(env, ids);
    env.objc.borrow_mut::<SKProductsRequestHostObject>(this).product_identifiers = ids;
    this
}

- (())setDelegate:(id)del {
    env.objc.borrow_mut::<SKProductsRequestHostObject>(this).delegate = del;
}

- (id)delegate {
    env.objc.borrow::<SKProductsRequestHostObject>(this).delegate
}

- (())start {
    log_dbg!("SKProductsRequest start");
    let delegate = env.objc.borrow::<SKProductsRequestHostObject>(this).delegate;
    if delegate != nil {
        let ids_set = env.objc.borrow::<SKProductsRequestHostObject>(this).product_identifiers;
        let mut products_vec: Vec<id> = Vec::new();
        if ids_set != nil {
            let all_objects: id = msg![env; ids_set allObjects];
            if all_objects != nil {
                let count: NSUInteger = msg![env; all_objects count];
                for i in 0..count {
                    let ident_id: id = msg![env; all_objects objectAtIndex:i];
                    let ident_str = to_rust_string(env, ident_id);
                    let price_val = get_item_price_in_meso(&ident_str);

                    let prod: id = msg_class![env; SKProduct alloc];
                    let prod: id = msg![env; prod init];

                    let num_str = from_rust_string(env, price_val.to_string());
                    let price_dec: id = msg_class![env; NSDecimalNumber decimalNumberWithString:num_str];
                    let loc: id = msg_class![env; NSLocale currentLocale];

                    retain(env, ident_id);
                    retain(env, price_dec);
                    retain(env, loc);

                    let prod_host = env.objc.borrow_mut::<SKProductHostObject>(prod);
                    prod_host.product_identifier = ident_id;
                    prod_host.price = price_dec;
                    prod_host.price_locale = loc;

                    products_vec.push(prod);
                }
            }
        }

        let products_array = ns_array::from_vec(env, products_vec);
        retain(env, products_array);

        let empty_invalid = ns_array::from_vec(env, Vec::new());
        retain(env, empty_invalid);

        let response: id = msg_class![env; SKProductsResponse alloc];
        let response: id = msg![env; response init];
        let resp_host = env.objc.borrow_mut::<SKProductsResponseHostObject>(response);
        resp_host.products = products_array;
        resp_host.invalid_product_identifiers = empty_invalid;

        let did_receive_sel = env.objc.register_host_selector("productsRequest:didReceiveResponse:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, delegate, did_receive_sel) {
            let _: () = crate::objc::msg_send(env, (delegate, did_receive_sel, this, response));
        }
        let did_finish_sel = env.objc.register_host_selector("requestDidFinish:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, delegate, did_finish_sel) {
            let _: () = crate::objc::msg_send(env, (delegate, did_finish_sel, this));
        }
        release(env, response);
    }
}

- (())cancel {}

- (())dealloc {
    let ids = env.objc.borrow_mut::<SKProductsRequestHostObject>(this).product_identifiers;
    release(env, ids);
    crate::objc::msg_super![env; this dealloc]
}

@end

@implementation SKProductsResponse: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SKProductsResponseHostObject {
        products: nil,
        invalid_product_identifiers: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)products {
    let p = env.objc.borrow::<SKProductsResponseHostObject>(this).products;
    if p != nil {
        p
    } else {
        msg_class![env; NSArray array]
    }
}

- (id)invalidProductIdentifiers {
    let inv = env.objc.borrow::<SKProductsResponseHostObject>(this).invalid_product_identifiers;
    if inv != nil {
        inv
    } else {
        msg_class![env; NSArray array]
    }
}

- (())dealloc {
    let (products, invalid) = {
        let host = env.objc.borrow::<SKProductsResponseHostObject>(this);
        (host.products, host.invalid_product_identifiers)
    };
    release(env, products);
    release(env, invalid);
    crate::objc::msg_super![env; this dealloc]
}

@end

@implementation SKPayment: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(SKPaymentHostObject {
        product_identifier: nil,
        quantity: 1,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)paymentWithProduct:(id)prod {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    if prod != nil {
        let ident: id = msg![env; prod productIdentifier];
        if ident != nil {
            retain(env, ident);
            env.objc.borrow_mut::<SKPaymentHostObject>(new).product_identifier = ident;
        }
    }
    autorelease(env, new)
}

+ (id)paymentWithProductIdentifier:(id)ident {
    let new: id = msg![env; this alloc];
    let new: id = msg![env; new init];
    if ident != nil {
        retain(env, ident);
        env.objc.borrow_mut::<SKPaymentHostObject>(new).product_identifier = ident;
    }
    autorelease(env, new)
}

- (id)productIdentifier {
    env.objc.borrow::<SKPaymentHostObject>(this).product_identifier
}

- (NSInteger)quantity {
    env.objc.borrow::<SKPaymentHostObject>(this).quantity
}

- (())dealloc {
    let ident = env.objc.borrow::<SKPaymentHostObject>(this).product_identifier;
    release(env, ident);
    crate::objc::msg_super![env; this dealloc]
}

@end

@implementation SKPaymentTransaction: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let date: id = msg_class![env; NSDate date];
    retain(env, date);
    let host_object = Box::new(SKPaymentTransactionHostObject {
        payment: nil,
        transaction_state: 1, // SKPaymentTransactionStatePurchased
        transaction_identifier: nil,
        transaction_date: date,
        error: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)payment {
    env.objc.borrow::<SKPaymentTransactionHostObject>(this).payment
}

- (NSInteger)transactionState {
    env.objc.borrow::<SKPaymentTransactionHostObject>(this).transaction_state
}

- (id)transactionIdentifier {
    env.objc.borrow::<SKPaymentTransactionHostObject>(this).transaction_identifier
}

- (id)transactionDate {
    env.objc.borrow::<SKPaymentTransactionHostObject>(this).transaction_date
}

- (id)error {
    env.objc.borrow::<SKPaymentTransactionHostObject>(this).error
}

- (id)originalTransaction {
    nil
}

- (())dealloc {
    let (payment, ident, date, error) = {
        let host = env.objc.borrow::<SKPaymentTransactionHostObject>(this);
        (host.payment, host.transaction_identifier, host.transaction_date, host.error)
    };
    release(env, payment);
    release(env, ident);
    release(env, date);
    release(env, error);
    crate::objc::msg_super![env; this dealloc]
}

@end

};
