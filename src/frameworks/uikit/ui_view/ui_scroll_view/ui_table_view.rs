/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITableView`.

use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::{ns_string::get_static_str, NSInteger, NSUInteger};
use crate::objc::{
    autorelease, id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes,
    release, retain, ClassExports, NSZonePtr,
};
use crate::Environment;

pub struct UITableViewHostObject {
    pub superclass: super::UIScrollViewHostObject,
    pub data_source: id,
    pub row_height: CGFloat,
    pub section_header_height: CGFloat,
    pub section_footer_height: CGFloat,
    pub style: NSInteger,
    pub editing: bool,
    pub cells: Vec<id>,
    pub cell_index_paths: Vec<(NSInteger, NSInteger)>,
    pub reusable_cells: Vec<id>,
    pub needs_reload: bool,
    pub is_reloading: bool,
    pub selected_index_path: id,
}
impl_HostObject_with_superclass!(UITableViewHostObject);
impl Default for UITableViewHostObject {
    fn default() -> Self {
        Self {
            superclass: Default::default(),
            data_source: nil,
            row_height: 44.0,
            section_header_height: 0.0,
            section_footer_height: 0.0,
            style: 0,
            editing: false,
            cells: Vec::new(),
            cell_index_paths: Vec::new(),
            reusable_cells: Vec::new(),
            needs_reload: true,
            is_reloading: false,
            selected_index_path: nil,
        }
    }
}

fn reload_data(env: &mut Environment, this: id) {
    let old_cells = {
        let host = env.objc.borrow_mut::<UITableViewHostObject>(this);
        if host.is_reloading {
            return;
        }
        host.is_reloading = true;
        host.needs_reload = false;
        host.cell_index_paths.clear();
        std::mem::take(&mut host.cells)
    };
    for cell in old_cells {
        let _: () = msg![env; cell removeFromSuperview];
        let reuse_id = env
            .objc
            .try_borrow::<crate::frameworks::uikit::ui_view::ui_table_view_cell::UITableViewCellHostObject>(cell)
            .map(|h| h.reuse_identifier)
            .unwrap_or(nil);
        if reuse_id != nil {
            env.objc.borrow_mut::<UITableViewHostObject>(this).reusable_cells.push(cell);
        } else {
            release(env, cell);
        }
    }

    let data_source = env.objc.borrow::<UITableViewHostObject>(this).data_source;
    if data_source == nil {
        env.objc.borrow_mut::<UITableViewHostObject>(this).is_reloading = false;
        return;
    }

    let num_sections_sel = env.objc.register_host_selector("numberOfSectionsInTableView:".to_string(), &mut env.mem);
    let num_sections: NSInteger = if env.objc.object_has_method(&env.mem, data_source, num_sections_sel) {
        crate::objc::msg_send(env, (data_source, num_sections_sel, this))
    } else {
        1
    };

    let bounds: CGRect = msg![env; this bounds];
    let frame: CGRect = msg![env; this frame];
    let width = if bounds.size.width > 0.0 {
        bounds.size.width
    } else if frame.size.width > 0.0 {
        frame.size.width
    } else {
        320.0
    };

    let default_row_height = env.objc.borrow::<UITableViewHostObject>(this).row_height;
    let default_row_height = if default_row_height > 0.0 { default_row_height } else { 44.0 };

    let delegate = env.objc.borrow::<super::UIScrollViewHostObject>(this).delegate;
    let height_sel = env.objc.register_host_selector("tableView:heightForRowAtIndexPath:".to_string(), &mut env.mem);
    let num_rows_sel = env.objc.register_host_selector("tableView:numberOfRowsInSection:".to_string(), &mut env.mem);
    let cell_sel = env.objc.register_host_selector("tableView:cellForRowAtIndexPath:".to_string(), &mut env.mem);

    let mut current_y: CGFloat = 0.0;
    let mut new_cells = Vec::new();
    let mut new_cell_index_paths = Vec::new();

    for sec in 0..num_sections {
        let num_rows: NSInteger = if env.objc.object_has_method(&env.mem, data_source, num_rows_sel) {
            crate::objc::msg_send(env, (data_source, num_rows_sel, this, sec))
        } else {
            0
        };

        for row in 0..num_rows {
            let index_path: id = msg_class![env; NSIndexPath indexPathForRow:row inSection:sec];
            let cell: id = crate::objc::msg_send(env, (data_source, cell_sel, this, index_path));
            if cell == nil {
                continue;
            }

            let row_h: CGFloat = if delegate != nil && env.objc.object_has_method(&env.mem, delegate, height_sel) {
                crate::objc::msg_send(env, (delegate, height_sel, this, index_path))
            } else if env.objc.object_has_method(&env.mem, data_source, height_sel) {
                crate::objc::msg_send(env, (data_source, height_sel, this, index_path))
            } else {
                default_row_height
            };

            let cell_frame = CGRect {
                origin: CGPoint { x: 0.0, y: current_y },
                size: CGSize { width, height: row_h },
            };
            let _: () = msg![env; cell setFrame:cell_frame];
            let _: () = msg![env; this addSubview:cell];
            let _: () = msg![env; cell setNeedsLayout];
            let _: () = msg![env; cell layoutIfNeeded];
            retain(env, cell);
            new_cells.push(cell);
            new_cell_index_paths.push((sec, row));
            current_y += row_h;
        }
    }

    let content_size = CGSize { width, height: current_y };
    let _: () = msg![env; this setContentSize:content_size];
    let host = env.objc.borrow_mut::<UITableViewHostObject>(this);
    host.cells = new_cells;
    host.cell_index_paths = new_cell_index_paths;
    host.is_reloading = false;
    let _: () = msg![env; this setNeedsDisplay];
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITableView: UIScrollView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITableViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame style:(NSInteger)style {
    let this: id = msg_super![env; this initWithFrame:frame];
    env.objc.borrow_mut::<UITableViewHostObject>(this).style = style;
    this
}

- (id)initWithFrame:(CGRect)frame {
    msg![env; this initWithFrame:frame style:0]
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    let style_key = get_static_str(env, "UIStyle");
    let style: NSInteger = msg![env; coder decodeIntegerForKey:style_key];
    env.objc.borrow_mut::<UITableViewHostObject>(this).style = style;
    let rh_key = get_static_str(env, "UIRowHeight");
    let rh: f64 = msg![env; coder decodeDoubleForKey:rh_key];
    if rh > 0.0 {
        env.objc.borrow_mut::<UITableViewHostObject>(this).row_height = rh as CGFloat;
    }
    let sh_key = get_static_str(env, "UISectionHeaderHeight");
    let sh: f64 = msg![env; coder decodeDoubleForKey:sh_key];
    if sh > 0.0 {
        env.objc.borrow_mut::<UITableViewHostObject>(this).section_header_height = sh as CGFloat;
    }
    let sf_key = get_static_str(env, "UISectionFooterHeight");
    let sf: f64 = msg![env; coder decodeDoubleForKey:sf_key];
    if sf > 0.0 {
        env.objc.borrow_mut::<UITableViewHostObject>(this).section_footer_height = sf as CGFloat;
    }
    let ds_key = get_static_str(env, "UIDataSource");
    let ds: id = msg![env; coder decodeObjectForKey:ds_key];
    if ds != nil {
        let _: () = msg![env; this setDataSource:ds];
    }
    let del_key = get_static_str(env, "UIDelegate");
    let del: id = msg![env; coder decodeObjectForKey:del_key];
    if del != nil {
        let _: () = msg![env; this setDelegate:del];
    }
    env.objc.borrow_mut::<UITableViewHostObject>(this).needs_reload = true;
    this
}

- (id)dataSource {
    env.objc.borrow::<UITableViewHostObject>(this).data_source
}

- (())setDataSource:(id)source {
    env.objc.borrow_mut::<UITableViewHostObject>(this).data_source = source;
    env.objc.borrow_mut::<UITableViewHostObject>(this).needs_reload = true;
    if source != nil {
        reload_data(env, this);
    }
}

- (CGFloat)rowHeight {
    env.objc.borrow::<UITableViewHostObject>(this).row_height
}

- (())setRowHeight:(CGFloat)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).row_height = height;
}

- (CGFloat)sectionHeaderHeight {
    env.objc.borrow::<UITableViewHostObject>(this).section_header_height
}

- (())setSectionHeaderHeight:(CGFloat)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).section_header_height = height;
}

- (CGFloat)sectionFooterHeight {
    env.objc.borrow::<UITableViewHostObject>(this).section_footer_height
}

- (())setSectionFooterHeight:(CGFloat)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).section_footer_height = height;
}

- (NSInteger)style {
    env.objc.borrow::<UITableViewHostObject>(this).style
}

- (())reloadData {
    log_dbg!("[(UITableView*){:?} reloadData]", this);
    reload_data(env, this);
}

- (())layoutSubviews {
    let _: () = msg_super![env; this layoutSubviews];
    let (needs_reload, is_reloading, has_ds) = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        (host.needs_reload, host.is_reloading, host.data_source != nil)
    };
    if needs_reload && !is_reloading && has_ds {
        reload_data(env, this);
    } else {
        let bounds: CGRect = msg![env; this bounds];
        let width = bounds.size.width;
        if width > 0.0 {
            let cells = env.objc.borrow::<UITableViewHostObject>(this).cells.clone();
            for cell in cells {
                let mut frame: CGRect = msg![env; cell frame];
                if frame.size.width != width {
                    frame.size.width = width;
                    let _: () = msg![env; cell setFrame:frame];
                    let _: () = msg![env; cell setNeedsLayout];
                }
                let _: () = msg![env; cell layoutIfNeeded];
            }
        }
    }
}

- (id)dequeueReusableCellWithIdentifier:(id)identifier {
    if identifier == nil {
        return nil;
    }
    let reusable = env.objc.borrow::<UITableViewHostObject>(this).reusable_cells.clone();
    let mut matching_idx = None;
    for (i, &cell) in reusable.iter().enumerate() {
        let cell_reuse_id = env
            .objc
            .try_borrow::<crate::frameworks::uikit::ui_view::ui_table_view_cell::UITableViewCellHostObject>(cell)
            .map(|h| h.reuse_identifier)
            .unwrap_or(nil);
        if cell_reuse_id != nil {
            let is_equal: bool = msg![env; cell_reuse_id isEqualToString:identifier];
            if is_equal {
                matching_idx = Some(i);
                break;
            }
        }
    }
    if let Some(i) = matching_idx {
        let cell = env.objc.borrow_mut::<UITableViewHostObject>(this).reusable_cells.swap_remove(i);
        let _: () = msg![env; cell prepareForReuse];
        autorelease(env, cell)
    } else {
        nil
    }
}

- (NSInteger)numberOfSections {
    let ds = env.objc.borrow::<UITableViewHostObject>(this).data_source;
    if ds != nil {
        let sel = env.objc.register_host_selector("numberOfSectionsInTableView:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, ds, sel) {
            return crate::objc::msg_send(env, (ds, sel, this));
        }
    }
    1
}

- (NSInteger)numberOfRowsInSection:(NSInteger)section {
    let ds = env.objc.borrow::<UITableViewHostObject>(this).data_source;
    if ds != nil {
        let sel = env.objc.register_host_selector("tableView:numberOfRowsInSection:".to_string(), &mut env.mem);
        if env.objc.object_has_method(&env.mem, ds, sel) {
            return crate::objc::msg_send(env, (ds, sel, this, section));
        }
    }
    0
}

- (())touchesEnded:(id)touches withEvent:(id)event {
    let was_dragging = env.objc.borrow::<super::UIScrollViewHostObject>(this).is_dragging;
    let _: () = msg_super![env; this touchesEnded:touches withEvent:event];
    if was_dragging {
        return;
    }
    let touch: id = msg![env; touches anyObject];
    if touch == nil {
        return;
    }
    let pt: CGPoint = msg![env; touch locationInView:this];
    let (cells, ips) = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        (host.cells.clone(), host.cell_index_paths.clone())
    };
    for (i, &cell) in cells.iter().enumerate() {
        let frame: CGRect = msg![env; cell frame];
        if pt.y >= frame.origin.y && pt.y <= frame.origin.y + frame.size.height {
            let (sec, row) = ips.get(i).copied().unwrap_or((0, i as NSInteger));
            let ip: id = msg_class![env; NSIndexPath indexPathForRow:row inSection:sec];
            let _: () = msg![env; this selectRowAtIndexPath:ip animated:true scrollPosition:0];
            let delegate = env.objc.borrow::<super::UIScrollViewHostObject>(this).delegate;
            let mut handled = false;
            if delegate != nil {
                let sel = env.objc.register_host_selector("tableView:didSelectRowAtIndexPath:".to_string(), &mut env.mem);
                if env.objc.object_has_method(&env.mem, delegate, sel) {
                    let _: () = crate::objc::msg_send(env, (delegate, sel, this, ip));
                    handled = true;
                }
            }
            if !handled {
                // If delegate did not handle didSelectRowAtIndexPath (e.g. PassiveSkillsPane),
                // check if the cell responds to useItem
                let use_item_sel = env.objc.register_host_selector("useItem".to_string(), &mut env.mem);
                if env.objc.object_has_method(&env.mem, cell, use_item_sel) {
                    log_dbg!("Cell {:?} responds to useItem, triggering useItem", cell);
                    let _: () = crate::objc::msg_send(env, (cell, use_item_sel));
                }
            }
            break;
        }
    }
}

- (id)cellForRowAtIndexPath:(id)indexPath {
    if indexPath == nil {
        return nil;
    }
    let row: NSInteger = msg![env; indexPath row];
    let section: NSInteger = msg![env; indexPath section];
    let host = env.objc.borrow::<UITableViewHostObject>(this);
    if let Some(pos) = host.cell_index_paths.iter().position(|&(s, r)| s == section && r == row) {
        host.cells[pos]
    } else if section == 0 && (row as usize) < host.cells.len() {
        host.cells[row as usize]
    } else {
        nil
    }
}

- (id)indexPathForSelectedRow {
    env.objc.borrow::<UITableViewHostObject>(this).selected_index_path
}

- (())selectRowAtIndexPath:(id)indexPath animated:(bool)animated scrollPosition:(NSInteger)scroll_pos {
    if indexPath == nil {
        return;
    }
    let row: NSInteger = msg![env; indexPath row];
    let section: NSInteger = msg![env; indexPath section];
    let cell = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        if let Some(pos) = host.cell_index_paths.iter().position(|&(s, r)| s == section && r == row) {
            Some(host.cells[pos])
        } else if section == 0 && (row as usize) < host.cells.len() {
            Some(host.cells[row as usize])
        } else {
            None
        }
    };

    let old_ip = env.objc.borrow_mut::<UITableViewHostObject>(this).selected_index_path;
    retain(env, indexPath);
    env.objc.borrow_mut::<UITableViewHostObject>(this).selected_index_path = indexPath;
    release(env, old_ip);

    if let Some(cell) = cell {
        let _: () = msg![env; cell setSelected:true animated:animated];
        let _: () = msg![env; this scrollToRowAtIndexPath:indexPath atScrollPosition:scroll_pos animated:animated];
    }
}

- (())deselectRowAtIndexPath:(id)indexPath animated:(bool)animated {
    if indexPath == nil {
        return;
    }
    let row: NSInteger = msg![env; indexPath row];
    let section: NSInteger = msg![env; indexPath section];
    let cell = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        if let Some(pos) = host.cell_index_paths.iter().position(|&(s, r)| s == section && r == row) {
            Some(host.cells[pos])
        } else if section == 0 && (row as usize) < host.cells.len() {
            Some(host.cells[row as usize])
        } else {
            None
        }
    };

    let old_ip = env.objc.borrow::<UITableViewHostObject>(this).selected_index_path;
    if old_ip != nil {
        let old_row: NSInteger = msg![env; old_ip row];
        let old_sec: NSInteger = msg![env; old_ip section];
        if old_row == row && old_sec == section {
            env.objc.borrow_mut::<UITableViewHostObject>(this).selected_index_path = nil;
            release(env, old_ip);
        }
    }

    if let Some(cell) = cell {
        let _: () = msg![env; cell setSelected:false animated:animated];
    }
}

- (())setSeparatorStyle:(NSInteger)_style {
    // no-op
}

- (NSInteger)separatorStyle {
    0
}

- (())setSeparatorColor:(id)_color {
    // no-op
}

- (())setAllowsSelection:(bool)_allows {
    // no-op
}

- (bool)allowsSelection {
    true
}

- (())deleteRowsAtIndexPaths:(id)_paths withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())reloadRowsAtIndexPaths:(id)_paths withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())insertRowsAtIndexPaths:(id)_paths withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())deleteSections:(id)_sections withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())insertSections:(id)_sections withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())reloadSections:(id)_sections withRowAnimation:(NSInteger)_animation {
    let _: () = msg![env; this reloadData];
}

- (())beginUpdates {
    // no-op
}

- (())endUpdates {
    let _: () = msg![env; this reloadData];
}

- (id)indexPathForCell:(id)cell {
    let mut cell = cell;
    let (cells, ips) = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        (host.cells.clone(), host.cell_index_paths.clone())
    };
    while cell != nil && cell != this {
        if let Some(pos) = cells.iter().position(|&c| c == cell) {
            let (sec, row) = ips.get(pos).copied().unwrap_or((0, pos as NSInteger));
            return msg_class![env; NSIndexPath indexPathForRow:row inSection:sec];
        }
        cell = msg![env; cell superview];
    }
    nil
}

- (())scrollToRowAtIndexPath:(id)indexPath atScrollPosition:(NSInteger)_pos animated:(bool)_anim {
    if indexPath == nil {
        return;
    }
    let row: NSInteger = msg![env; indexPath row];
    let section: NSInteger = msg![env; indexPath section];
    let cell = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        if let Some(pos) = host.cell_index_paths.iter().position(|&(s, r)| s == section && r == row) {
            Some(host.cells[pos])
        } else if section == 0 && (row as usize) < host.cells.len() {
            Some(host.cells[row as usize])
        } else {
            None
        }
    };
    if let Some(cell) = cell {
        let frame: CGRect = msg![env; cell frame];
        let offset = CGPoint { x: 0.0, y: frame.origin.y };
        let _: () = msg![env; this setContentOffset:offset];
    }
}

- (CGRect)rectForRowAtIndexPath:(id)indexPath {
    if indexPath == nil {
        return CGRect::default();
    }
    let row: NSInteger = msg![env; indexPath row];
    let section: NSInteger = msg![env; indexPath section];
    let cell = {
        let host = env.objc.borrow::<UITableViewHostObject>(this);
        if let Some(pos) = host.cell_index_paths.iter().position(|&(s, r)| s == section && r == row) {
            Some(host.cells[pos])
        } else if section == 0 && (row as usize) < host.cells.len() {
            Some(host.cells[row as usize])
        } else {
            None
        }
    };
    if let Some(cell) = cell {
        msg![env; cell frame]
    } else {
        CGRect::default()
    }
}

- (())setEditing:(bool)editing animated:(bool)_animated {
    let _: () = msg![env; this setEditing:editing];
}

- (())setEditing:(bool)editing {
    env.objc.borrow_mut::<UITableViewHostObject>(this).editing = editing;
}

- (bool)isEditing {
    env.objc.borrow::<UITableViewHostObject>(this).editing
}

- (id)indexPathsForVisibleRows {
    let ips = env.objc.borrow::<UITableViewHostObject>(this).cell_index_paths.clone();
    let arr = msg_class![env; NSMutableArray arrayWithCapacity:(ips.len() as NSUInteger)];
    for (sec, row) in ips {
        let ip: id = msg_class![env; NSIndexPath indexPathForRow:row inSection:sec];
        let _: () = msg![env; arr addObject:ip];
    }
    arr
}

- (id)visibleCells {
    let cells = env.objc.borrow::<UITableViewHostObject>(this).cells.clone();
    let arr = msg_class![env; NSMutableArray arrayWithCapacity:(cells.len() as NSUInteger)];
    for cell in cells {
        let _: () = msg![env; arr addObject:cell];
    }
    arr
}

- (())dealloc {
    let (cells, reusable_cells, selected_index_path) = {
        let host = env.objc.borrow_mut::<UITableViewHostObject>(this);
        (
            std::mem::take(&mut host.cells),
            std::mem::take(&mut host.reusable_cells),
            std::mem::take(&mut host.selected_index_path),
        )
    };
    for cell in cells {
        release(env, cell);
    }
    for cell in reusable_cells {
        release(env, cell);
    }
    release(env, selected_index_path);
    msg_super![env; this dealloc]
}

@end

@implementation _touchHLE_MockTableViewDataSource: NSObject

- (NSInteger)numberOfSectionsInTableView:(id)_tableView {
    1
}

- (NSInteger)tableView:(id)_tableView numberOfRowsInSection:(NSInteger)_section {
    3
}

- (id)tableView:(id)_tableView cellForRowAtIndexPath:(id)_indexPath {
    let cell: id = msg_class![env; UITableViewCell alloc];
    let cell: id = msg![env; cell initWithFrame:(CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: 320.0, height: 44.0 },
    })];
    autorelease(env, cell)
}

@end

@implementation _touchHLE_MockEmptyTableViewDataSource: NSObject

- (NSInteger)numberOfSectionsInTableView:(id)_tableView {
    1
}

- (NSInteger)tableView:(id)_tableView numberOfRowsInSection:(NSInteger)_section {
    0
}

- (id)tableView:(id)_tableView cellForRowAtIndexPath:(id)_indexPath {
    nil
}

@end

@implementation _touchHLE_MockReentrantTableViewDataSource: NSObject

- (NSInteger)numberOfSectionsInTableView:(id)_tableView {
    1
}

- (NSInteger)tableView:(id)_tableView numberOfRowsInSection:(NSInteger)_section {
    2
}

- (id)tableView:(id)_tableView cellForRowAtIndexPath:(id)_indexPath {
    // Attempt re-entrant reloadData and layoutSubviews during cellForRowAtIndexPath:
    let _: () = msg![env; _tableView reloadData];
    let _: () = msg![env; _tableView layoutSubviews];
    let cell: id = msg_class![env; UITableViewCell alloc];
    let cell: id = msg![env; cell initWithFrame:(CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width: 320.0, height: 44.0 },
    })];
    autorelease(env, cell)
}

@end

};
