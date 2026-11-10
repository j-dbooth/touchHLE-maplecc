/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIViewController`.
//!
//! Resources:
//! - [View Controller Programming Guide for iOS (Legacy)](https://developer.apple.com/library/archive/documentation/WindowsViews/Conceptual/ViewControllerPGforiOSLegacy/BasicViewControllers/BasicViewControllers.html)

use crate::abi::CallFromHost;
use crate::frameworks::core_graphics::CGRect;
use crate::frameworks::foundation::ns_objc_runtime::NSStringFromClass;
use crate::frameworks::foundation::ns_string::{from_rust_string, get_static_str, to_rust_string};
use crate::frameworks::foundation::NSInteger;
use crate::frameworks::uikit::ui_application::{
    UIInterfaceOrientation, UIInterfaceOrientationPortrait,
};
use crate::frameworks::uikit::ui_view::set_view_controller;
use crate::objc::{
    id, msg, msg_class, msg_super, nil, objc_classes, release, retain, todo_objc_setter, Class, ClassExports,
    HostObject, NSZonePtr,
};
use crate::Environment;

pub mod ui_navigation_controller;

#[derive(Default)]
struct UIViewControllerHostObject {
    /// The root view.
    /// `UIView*`
    view: id,
    /// Nib name to be used at the load
    /// of the root view, may be nil.
    /// `NSString*`
    nib_name: id,
    /// Bundle to be used for load
    /// of the nib by name, may be nil.
    /// `NSBundle*`
    bundle: id,
    /// Presented modal view controller
    /// `UIViewController*`
    modal_view_controller: id,
    /// Parent / presenting view controller
    /// `UIViewController*`
    parent_view_controller: id,
    /// `NSString*`
    title: id,
    /// Whether viewDidLoad has already been invoked for the current view hierarchy.
    view_did_load_called: bool,
}
impl HostObject for UIViewControllerHostObject {}

type UIModalTransitionStyle = NSInteger;

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIViewController: UIResponder

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIViewControllerHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// TODO: this should be a designated initializer
- (id)initWithNibName:(id)nib_name // NSString *
               bundle:(id)bundle { // NSBundle *
    retain(env, nib_name);
    retain(env, bundle);

    log_dbg!("[(UIViewController*){:?} initWithNibName:{:?} bundle:{:?}]", this, nib_name, bundle);

    env.objc.borrow_mut::<UIViewControllerHostObject>(this).nib_name = nib_name;
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).bundle = bundle;

    this
}

- (id)initWithCoder:(id)coder {
    let key_ns_string = get_static_str(env, "UIView");
    let view: id = msg![env; coder decodeObjectForKey:key_ns_string];
    if view != nil {
        () = msg![env; this setView:view];
    }

    let nib_key = get_static_str(env, "UINibName");
    let nib_name: id = msg![env; coder decodeObjectForKey:nib_key];
    if nib_name != nil {
        retain(env, nib_name);
        env.objc.borrow_mut::<UIViewControllerHostObject>(this).nib_name = nib_name;
    }

    let bundle_key = get_static_str(env, "UINibBundle");
    let bundle: id = msg![env; coder decodeObjectForKey:bundle_key];
    if bundle != nil {
        retain(env, bundle);
        env.objc.borrow_mut::<UIViewControllerHostObject>(this).bundle = bundle;
    }

    let title_key = get_static_str(env, "UITitle");
    let title: id = msg![env; coder decodeObjectForKey:title_key];
    if title != nil {
        retain(env, title);
        env.objc.borrow_mut::<UIViewControllerHostObject>(this).title = title;
    }

    this
}

- (())dealloc {
    let &UIViewControllerHostObject { view, nib_name, bundle, modal_view_controller, title, .. } = env.objc.borrow(this);

    if view != nil {
        set_view_controller(env, view, nil);
    }
    release(env, view);
    release(env, nib_name);
    release(env, bundle);
    release(env, modal_view_controller);
    release(env, title);

    env.objc.dealloc_object(this, &mut env.mem);
}

- (())loadView {
    let bundle: id = env.objc.borrow::<UIViewControllerHostObject>(this).bundle;
    let bundle: id = if bundle == nil {
        msg_class![env; NSBundle mainBundle]
    } else {
        bundle
    };

    let nib_name: id = get_nib_name(env, this, bundle);
    if nib_name != nil {
        // If we do have nib name, try to load it!
        log_dbg!(
            "Load {:?} view controller's view by nib, using name {}", this, to_rust_string(env, nib_name)
        );

        let nib: id = msg_class![env; UINib nibWithNibName:nib_name bundle:bundle];
        release(env, nib_name);

        // The NIB's File's Owner will be substituted by `this`,
        // implicitly loading the view as well
        let _: id = msg![env; nib instantiateWithOwner:this options:nil];

        let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
        // Having nil view at this point probably mean that
        // out nib's parsing is wrong.
        // Also we assume here the case of a "detached nib file"
        // TODO: support "integrated nib file"
        assert!(view != nil);

        return;
    };

    // As a last resort, use plain UIVIew for the root view
    let class: Class = msg![env; this class];
    log!("Unable to load {:?} {} view controller's view by nib, using plain UIView", this, env.objc.get_class_name(class).to_string());
    let view: id = msg_class![env; UIView alloc];
    // Docs are saying that "an empty UIView" is created,
    // but testing reveals that frame matches the screen one
    // (at least on the simulator)
    let screen: id = msg_class![env; UIScreen mainScreen];
    let app_frame: CGRect = msg![env; screen applicationFrame];
    let view: id = msg![env; view initWithFrame:app_frame];
    () = msg![env; this setView:view];
}

- (())setView:(id)new_view { // UIView*
    let host_obj = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
    if new_view == nil {
        host_obj.view_did_load_called = false;
    }
    let old_view = std::mem::replace(&mut host_obj.view, new_view);
    if old_view != nil {
        set_view_controller(env, old_view, nil);
    }
    if new_view != nil {
        set_view_controller(env, new_view, this);
    }
    retain(env, new_view);
    release(env, old_view);
}
- (id)view {
    let view = env.objc.borrow::<UIViewControllerHostObject>(this).view;
    if view == nil {
        () = msg![env; this loadView];
    }
    let (view, should_call_viewDidLoad) = {
        let host = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
        let should = !host.view_did_load_called && host.view != nil;
        if should {
            host.view_did_load_called = true;
        }
        (host.view, should)
    };
    if should_call_viewDidLoad {
        () = msg![env; this viewDidLoad];
    }
    view
}

- (())awakeFromNib {
    let _: () = msg_super![env; this awakeFromNib];
    let should_call_viewDidLoad = {
        let host = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
        let should = !host.view_did_load_called && host.view != nil;
        if should {
            host.view_did_load_called = true;
        }
        should
    };
    if should_call_viewDidLoad {
        () = msg![env; this viewDidLoad];
    }
}

// Usually overridden by the application
- (())viewDidLoad {
    log_dbg!("[(UIViewController*){:?} viewDidLoad]", this);
}
- (())viewWillAppear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewWillAppear:{}]", this, animated);
}
- (())viewDidAppear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewDidAppear:{}]", this, animated);
}
- (())viewWillDisappear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewWillDisappear:{}]", this, animated);
}
- (())viewDidDisappear:(bool)animated {
    log_dbg!("[(UIViewController*){:?} viewDidDisappear:{}]", this, animated);
}

- (id)title {
    env.objc.borrow::<UIViewControllerHostObject>(this).title
}
- (())setTitle:(id)title { // NSString *
    retain(env, title);
    let old_title = env.objc.borrow_mut::<UIViewControllerHostObject>(this).title;
    env.objc.borrow_mut::<UIViewControllerHostObject>(this).title = title;
    release(env, old_title);
}
- (())setEditing:(bool)editing {
    todo_objc_setter!(this, editing);
}
- (())setWantsFullScreenLayout:(bool)wants {
    todo_objc_setter!(this, wants);
}
- (())setHidesBottomBarWhenPushed:(bool)hides {
    todo_objc_setter!(this, hides);
}
- (())setModalTransitionStyle:(UIModalTransitionStyle)style {
    todo_objc_setter!(this, style);
}

- (bool)isViewLoaded {
    let host = env.objc.borrow::<UIViewControllerHostObject>(this);
    host.view != nil
}

- (id)modalViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).modal_view_controller
}

- (id)presentedViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).modal_view_controller
}

- (id)parentViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).parent_view_controller
}

- (id)presentingViewController {
    env.objc.borrow::<UIViewControllerHostObject>(this).parent_view_controller
}

- (())presentModalViewController:(id)modal animated:(bool)animated {
    if modal == nil {
        return;
    }
    retain(env, modal);
    let old_modal = {
        let host = env.objc.borrow_mut::<UIViewControllerHostObject>(this);
        let old = host.modal_view_controller;
        host.modal_view_controller = modal;
        old
    };
    if old_modal != nil {
        release(env, old_modal);
    }
    {
        let modal_host = env.objc.borrow_mut::<UIViewControllerHostObject>(modal);
        modal_host.parent_view_controller = this;
    }

    () = msg![env; this viewWillDisappear:animated];
    () = msg![env; modal viewWillAppear:animated];

    let this_view: id = msg![env; this view];
    let modal_view: id = msg![env; modal view];

    if modal_view != nil {
        let window: id = if this_view != nil { msg![env; this_view window] } else { nil };
        if window != nil {
            () = msg![env; window addSubview:modal_view];
        } else if this_view != nil {
            () = msg![env; this_view addSubview:modal_view];
        }
    }

    () = msg![env; this viewDidDisappear:animated];
    () = msg![env; modal viewDidAppear:animated];
}

- (())dismissModalViewControllerAnimated:(bool)animated {
    let (presenter, modal) = {
        let host = env.objc.borrow::<UIViewControllerHostObject>(this);
        if host.modal_view_controller != nil {
            (this, host.modal_view_controller)
        } else if host.parent_view_controller != nil {
            (host.parent_view_controller, this)
        } else {
            log!("dismissModalViewControllerAnimated: called with no modal controller on {:?}", this);
            return;
        }
    };

    () = msg![env; modal viewWillDisappear:animated];
    () = msg![env; presenter viewWillAppear:animated];

    let modal_view: id = msg![env; modal view];
    if modal_view != nil {
        () = msg![env; modal_view removeFromSuperview];
    }

    env.objc.borrow_mut::<UIViewControllerHostObject>(presenter).modal_view_controller = nil;
    env.objc.borrow_mut::<UIViewControllerHostObject>(modal).parent_view_controller = nil;
    release(env, modal);

    () = msg![env; modal viewDidDisappear:animated];
    () = msg![env; presenter viewDidAppear:animated];
}

- (())presentViewController:(id)viewController animated:(bool)animated completion:(id)completion {
    () = msg![env; this presentModalViewController:viewController animated:animated];
    if completion != nil {
        let invoke_addr: u32 = env.mem.read(crate::mem::ConstPtr::<u32>::from_bits(completion.to_bits() + 12));
        let invoke = crate::abi::GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let (): () = invoke.call_from_host(env, (completion,));
    }
}

- (())dismissViewControllerAnimated:(bool)animated completion:(id)completion {
    () = msg![env; this dismissModalViewControllerAnimated:animated];
    if completion != nil {
        let invoke_addr: u32 = env.mem.read(crate::mem::ConstPtr::<u32>::from_bits(completion.to_bits() + 12));
        let invoke = crate::abi::GuestFunction::from_addr_with_thumb_bit(invoke_addr);
        let (): () = invoke.call_from_host(env, (completion,));
    }
}

- (())dismissMoviePlayerViewControllerAnimated {
    () = msg![env; this dismissModalViewControllerAnimated:true];
}

- (bool)shouldAutorotateToInterfaceOrientation:(UIInterfaceOrientation)interface_orientation {
    interface_orientation == UIInterfaceOrientationPortrait
}

// UIResponder implementation
// From the Apple UIView docs regarding [UIResponder nextResponder]:
// "UIViewController similarly implements the method
// and returns its view’s superview."
// https://developer.apple.com/documentation/uikit/uiresponder/next?language=objc
- (id)nextResponder {
    let view = msg![env; this view];
    let next_responder = msg![env; view superview];
    log_dbg!("[(UIView*){:?} nextResponder] => {:?}", this, next_responder);
    next_responder
}

@end

};

/// A helper function to resolve suitable NIB name for a `view_controller`
/// in the `bundle`. Returns nil if fails.
///
/// Note: It's a responsibility of a caller to release the returned name
/// if not-nil!
fn get_nib_name(env: &mut Environment, view_controller: id, bundle: id) -> id {
    let provider_nib_name: id = env
        .objc
        .borrow::<UIViewControllerHostObject>(view_controller)
        .nib_name;
    if provider_nib_name != nil {
        // TODO: it's not clear how to handle situation when
        // provided nib name do not exist in the bundle.
        // It probably means that our bundle resource loading
        // is faulty, to check
        assert!(check_nib_exists(env, bundle, provider_nib_name));

        retain(env, provider_nib_name);
        return provider_nib_name;
    };

    let class: Class = msg![env; view_controller class];
    let class_name: id = NSStringFromClass(env, class);
    let class_name_str = to_rust_string(env, class_name);

    if let Some(name) = class_name_str.strip_suffix("Controller") {
        let ns_name: id = from_rust_string(env, name.to_string());
        if check_nib_exists(env, bundle, ns_name) {
            release(env, class_name);
            return ns_name;
        }
    }

    if check_nib_exists(env, bundle, class_name) {
        class_name
    } else {
        release(env, class_name);
        nil
    }
}

/// A helper function to check if `nib_name` NIB actually
/// existing in the `bundle`
fn check_nib_exists(env: &mut Environment, bundle: id, nib_name: id) -> bool {
    let type_: id = get_static_str(env, "nib");
    let res: id = msg![env; bundle pathForResource:nib_name ofType:type_];
    res != nil
}
