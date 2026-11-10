/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSOperation` and `NSOperationQueue`.

use crate::frameworks::foundation::NSInteger;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports};

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSOperation: NSObject

- (())start {
    () = msg![env; this main];
}

- (())main {}

- (())cancel {}

- (bool)isCancelled {
    false
}

- (bool)isExecuting {
    false
}

- (bool)isFinished {
    true
}

- (bool)isConcurrent {
    false
}

- (bool)isReady {
    true
}

@end

@implementation NSOperationQueue: NSObject

+ (id)mainQueue {
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

+ (id)currentQueue {
    msg_class![env; NSOperationQueue mainQueue]
}

- (())addOperation:(id)op {
    log_dbg!("[(NSOperationQueue*){:?} addOperation:{:?}]", this, op);
    if op != nil {
        () = msg![env; op start];
    }
}

- (())cancelAllOperations {
    // no-op
}

- (())waitUntilAllOperationsAreFinished {
    // no-op
}

- (())setMaxConcurrentOperationCount:(NSInteger)_count {
    // no-op
}

- (NSInteger)maxConcurrentOperationCount {
    1
}

@end

};
