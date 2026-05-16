use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use qrcode::{EcLevel, QrCode};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Document, Element};

#[allow(warnings)]
mod client {
    include!(concat!(env!("OUT_DIR"), "/client.rs"));
}

use client::{
    types::{BookSlotRequest, BookingPageResponse, CreateSlotRequest},
    Client,
};

fn document() -> Document {
    window().unwrap().document().unwrap()
}

fn origin() -> String {
    window().unwrap().location().origin().unwrap()
}

fn pathname() -> String {
    window().unwrap().location().pathname().unwrap()
}

fn api_client() -> Client {
    Client::new(&origin())
}

fn uuid_to_b64url(uuid: uuid::Uuid) -> String {
    URL_SAFE_NO_PAD.encode(uuid.as_bytes())
}

fn b64url_to_uuid(s: &str) -> Option<uuid::Uuid> {
    let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
    uuid::Uuid::from_slice(&bytes).ok()
}

// Read token from /<token> path or ?uuid=... query string.
// Accepts base64url (22 chars) or standard UUID (36 chars).
fn parse_uuid_param() -> Option<String> {
    // Check path segment first: /<token>
    let path = pathname();
    let segment = path.trim_start_matches('/');
    if segment.len() == 22 && !segment.contains('/') {
        return b64url_to_uuid(segment).map(|u| u.to_string());
    }

    // Fall back to ?uuid= query param
    let search = window().unwrap().location().search().unwrap_or_default();
    let raw = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            if k == "uuid" { Some(v.to_string()) } else { None }
        })?;

    if raw.len() == 22 {
        Some(b64url_to_uuid(&raw)?.to_string())
    } else {
        Some(raw)
    }
}

// Detect view vs edit from pathname
fn parse_path() -> Option<(String, bool)> {
    let uuid = parse_uuid_param()?;
    let path = pathname();
    let is_edit = path.trim_end_matches('/') == "/edit";
    Some((uuid, is_edit))
}

fn parse_admin_uuid() -> Option<String> {
    if pathname().trim_end_matches('/') != "/admin" {
        return None;
    }
    parse_uuid_param()
}

#[wasm_bindgen(start)]
pub fn main() {
    if let Some(el) = document().get_element_by_id("no-js-warning") {
        el.remove();
    }
    // Admin page
    if let Some(admin_uuid) = parse_admin_uuid() {
        // Wire create-slot form once
        if let Some(form) = document().get_element_by_id("create-slot-form") {
            let admin_clone = admin_uuid.clone();
            let onsubmit = Closure::<dyn FnMut(_)>::new(move |e: web_sys::Event| {
                e.prevent_default();
                let doc = document();
                let label = doc.get_element_by_id("slot-label")
                    .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .map(|el| el.value())
                    .unwrap_or_default();
                let max_bookings: i32 = doc.get_element_by_id("slot-max")
                    .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .and_then(|el| el.value().parse().ok())
                    .unwrap_or(1);
                let admin_uuid = admin_clone.clone();
                spawn_local(async move {
                    let client = api_client();
                    let Ok(admin) = admin_uuid.parse::<uuid::Uuid>() else { return; };
                    let body = CreateSlotRequest { label, max_bookings };
                    match client.create_slot(&admin, &body).await {
                        Ok(_) => render_admin(admin_uuid).await,
                        Err(_) => {},
                    }
                });
            });
            form.add_event_listener_with_callback("submit", onsubmit.as_ref().unchecked_ref()).unwrap();
            onsubmit.forget();
        }

        spawn_local(async move { render_admin(admin_uuid).await });
        return;
    }

    let Some((uuid_str, is_edit)) = parse_path() else {
        show_invalid_code();
        return;
    };
    let Ok(uuid) = uuid_str.parse::<uuid::Uuid>() else {
        show_invalid_code();
        return;
    };

    if is_edit {
        spawn_local(async move { render_edit(uuid).await });
    } else {
        // Block the edit button until data has loaded
        if let Some(btn) = document().get_element_by_id("view-edit-btn") {
            let onclick = Closure::<dyn FnMut(_)>::new(|e: web_sys::MouseEvent| {
                e.prevent_default();
                window().unwrap().alert_with_message("Bitte warte, bis die Daten geladen sind.").unwrap();
            });
            btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
            onclick.forget();
        }
        spawn_local(async move { render_view(uuid).await });
    }
}

// ── View page ────────────────────────────────────────────────────────────────

async fn render_view(uuid: uuid::Uuid) {
    let client = api_client();
    match client.get_booking_page(&uuid).await {
        Ok(resp) => {
            let data = resp.into_inner();
            mount_view(uuid, &data);
        }
        Err(_) => show_invalid_code(),
    }
}

fn mount_view(uuid: uuid::Uuid, data: &BookingPageResponse) {
    let doc = document();
    let is_booked = data.current_slot_id.is_some();

    if let Some(el) = doc.query_selector(".not-booked").ok().flatten() {
        el.set_class_name(if is_booked { "not-booked" } else { "not-booked visible" });
    }
    if let Some(el) = doc.query_selector(".booked").ok().flatten() {
        el.set_class_name(if is_booked { "booked visible" } else { "booked" });
    }

    if is_booked {
        let name_val = if data.user_name.is_empty() { "—" } else { &data.user_name };
        if let Some(el) = doc.get_element_by_id("view-name") {
            el.set_text_content(Some(name_val));
            el.set_class_name("info-value");
        }

        let email_val = if data.user_email.is_empty() { "—" } else { &data.user_email };
        if let Some(el) = doc.get_element_by_id("view-email") {
            el.set_text_content(Some(email_val));
            el.set_class_name("info-value");
        }

        let slot_val = data
            .current_slot_id
            .and_then(|id| data.slots.iter().find(|s| s.id == id))
            .map(|s| s.label.as_str())
            .unwrap_or("Unbekannt")
            .to_string();
        if let Some(el) = doc.get_element_by_id("view-slot") {
            el.set_text_content(Some(&slot_val));
            el.set_class_name("info-value");
        }
    } else {
        let edit_href = format!("/edit?uuid={}", uuid_to_b64url(uuid));
        if let Some(btn) = doc.get_element_by_id("view-edit-btn") {
            let new_btn = btn.clone_node_with_deep(true).unwrap();
            btn.parent_node().unwrap().replace_child(&new_btn, &btn).unwrap();
            let new_btn: Element = new_btn.unchecked_into();
            new_btn.set_attribute("href", &edit_href).unwrap();
        }
    }
}

// ── Edit page ────────────────────────────────────────────────────────────────

async fn render_edit(uuid: uuid::Uuid) {
    let client = api_client();
    match client.get_booking_page(&uuid).await {
        Ok(resp) => {
            let data = resp.into_inner();
            if data.current_slot_id.is_some() {
                window().unwrap().location().set_href(&format!("/{}", uuid_to_b64url(uuid))).unwrap();
                return;
            }
            mount_edit(uuid, &data);
        }
        Err(_) => show_invalid_code(),
    }
}

fn mount_edit(uuid: uuid::Uuid, data: &BookingPageResponse) {
    let doc = document();

    // Fill name and email inputs
    if let Some(el) = doc.get_element_by_id("name")
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        el.set_value(&data.user_name);
    }
    if let Some(el) = doc.get_element_by_id("email")
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        el.set_value(&data.user_email);
    }

    // Populate slot cards
    let slots_grid = doc.get_element_by_id("slots-grid").unwrap();
    slots_grid.set_inner_html("");
    for slot in &data.slots {
        let available = slot.current_bookings < slot.max_bookings;
        let spots_left = slot.max_bookings - slot.current_bookings;
        let is_selected = data.current_slot_id == Some(slot.id);

        let id_str = format!("slot-{}", slot.id);
        let value_str = slot.id.to_string();

        let input = create_el(&doc, "input", &[
            ("type", "radio"),
            ("name", "slot"),
            ("id", &id_str),
            ("value", &value_str),
            ("class", "slot-radio"),
        ]);
        if is_selected { input.set_attribute("checked", "checked").unwrap(); }
        if !available { input.set_attribute("disabled", "disabled").unwrap(); }

        let mut card_class = "slot-card".to_string();
        if is_selected { card_class.push_str(" slot-card--selected"); }
        if !available { card_class.push_str(" slot-card--disabled"); }

        let card = create_el(&doc, "label", &[("for", &id_str), ("class", &card_class)]);

        let name_span = create_el(&doc, "span", &[("class", "slot-name")]);
        name_span.set_text_content(Some(&slot.label));

        let spots_text = if available {
            format!("{} von {} Plätzen verfügbar", spots_left, slot.max_bookings)
        } else {
            "Ausgebucht".to_string()
        };
        let spots_span = create_el(&doc, "span", &[("class", "slot-spots")]);
        spots_span.set_text_content(Some(&spots_text));

        card.append_child(&name_span).unwrap();
        card.append_child(&spots_span).unwrap();
        slots_grid.append_child(&input).unwrap();
        slots_grid.append_child(&card).unwrap();
    }

    // Wire up submit
    let form = doc.get_element_by_id("edit-form").unwrap();
    let uuid_str = uuid.to_string();
    let onsubmit = Closure::<dyn FnMut(_)>::new(move |e: web_sys::Event| {
        e.prevent_default();
        let doc = document();

        let name = input_value(&doc, "name");
        let email = input_value(&doc, "email");
        let slot_id: Option<i64> = doc
            .query_selector("input[name='slot']:checked")
            .ok()
            .flatten()
            .and_then(|el| el.get_attribute("value"))
            .and_then(|v| v.parse().ok());

        let Some(slot_id) = slot_id else {
            set_error("Bitte einen Slot auswählen.");
            return;
        };

        let uuid_str = uuid_str.clone();
        let name = name.clone();
        let email = email.clone();

        spawn_local(async move {
            let Ok(uuid) = uuid_str.parse::<uuid::Uuid>() else { return; };
            let client = api_client();
            let body = BookSlotRequest { name, email, slot_id };
            match client.book_slot(&uuid, &body).await {
                Ok(_) => {
                    let Ok(u) = uuid_str.parse::<uuid::Uuid>() else { return; };
                    window().unwrap().location().set_href(&format!("/{}", uuid_to_b64url(u))).unwrap();
                }
                Err(client::Error::ErrorResponse(e)) => {
                    set_error(&e.into_inner().message);
                }
                Err(_) => {
                    set_error("Ein Fehler ist aufgetreten. Bitte versuche es erneut.");
                }
            }
        });
    });

    form.add_event_listener_with_callback("submit", onsubmit.as_ref().unchecked_ref()).unwrap();
    onsubmit.forget();

    // Wire radio cards to update selected styling
    wire_slot_cards();
}

fn wire_slot_cards() {
    let doc = document();
    let inputs = doc.query_selector_all("input.slot-radio").unwrap();
    for i in 0..inputs.length() {
        let input = inputs.item(i).unwrap();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let doc = document();
            // Remove selected class from all cards
            let cards = doc.query_selector_all(".slot-card").unwrap();
            for j in 0..cards.length() {
                let card = cards.item(j).unwrap();
                let card: Element = card.unchecked_into();
                let cls = card.class_name();
                card.set_class_name(&cls.replace(" slot-card--selected", ""));
            }
            // Add selected class to the card matching checked input
            if let Ok(Some(checked)) = doc.query_selector("input.slot-radio:checked") {
                if let Some(id) = checked.get_attribute("id") {
                    if let Ok(Some(label)) = doc.query_selector(&format!("label[for='{}']", id)) {
                        let label: Element = label.unchecked_into();
                        let cls = label.class_name();
                        if !cls.contains("slot-card--selected") {
                            label.set_class_name(&format!("{} slot-card--selected", cls));
                        }
                    }
                }
            }
        });
        input
            .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

// ── Admin page ───────────────────────────────────────────────────────────────

async fn render_admin(admin_uuid: String) {
    let client = api_client();
    let Ok(admin_uuid_parsed) = admin_uuid.parse::<uuid::Uuid>() else {
        show_error("Invalid admin link.");
        return;
    };

    let users = match client.list_users(&admin_uuid_parsed).await {
        Ok(resp) => resp.into_inner(),
        Err(_) => { show_error("Unauthorized or server error."); return; }
    };
    let slots = match client.list_slots(&admin_uuid_parsed).await {
        Ok(resp) => resp.into_inner(),
        Err(_) => { show_error("Unauthorized or server error."); return; }
    };

    mount_admin(&admin_uuid, users, slots);
}

fn mount_admin(admin_uuid: &str, users: Vec<client::types::User>, slots: Vec<client::types::Slot>) {
    let doc = document();
    let admin_uuid = admin_uuid.to_string();

    // ── Users tbody ───────────────────────────────────────────────────────────
    let users_tbody = doc.get_element_by_id("users-tbody").unwrap();
    users_tbody.set_inner_html("");

    if users.is_empty() {
        let tr = create_el(&doc, "tr", &[]);
        let td = create_el(&doc, "td", &[("colspan", "5"), ("class", "admin-empty")]);
        td.set_text_content(Some("Keine Benutzer vorhanden."));
        tr.append_child(&td).unwrap();
        users_tbody.append_child(&tr).unwrap();
    } else {
        for user in &users {
            let tr = create_el(&doc, "tr", &[]);
            let token = user.token.to_string();
            let token_b64 = token.parse::<uuid::Uuid>()
                .map(uuid_to_b64url)
                .unwrap_or_else(|_| token.clone());
            let slot_label = user.slot_label.as_deref().unwrap_or("—");

            // Token cell (not editable)
            let td_token = create_el(&doc, "td", &[]);
            td_token.set_text_content(Some(&token_b64));
            tr.append_child(&td_token).unwrap();

            // Name + email cells (double-click to edit)
            for (field_val, field_name) in [
                (user.name.as_str(), "name"),
                (user.email.as_str(), "email"),
            ] {
                let td = create_el(&doc, "td", &[("title", "Doppelklick zum Bearbeiten"), ("style", "cursor:pointer")]);
                td.set_text_content(Some(field_val));
                let admin_clone = admin_uuid.clone();
                let token_clone = token.clone();
                let current_name = user.name.clone();
                let current_email = user.email.clone();
                let field_name = field_name.to_string();
                let td_el = td.clone();
                let ondblclick = Closure::<dyn FnMut()>::new(move || {
                    let current = td_el.text_content().unwrap_or_default();
                    let input: web_sys::HtmlInputElement = document()
                        .create_element("input").unwrap()
                        .dyn_into().unwrap();
                    input.set_value(&current);
                    input.set_class_name("field-input admin-inline-input");
                    td_el.set_text_content(None);
                    td_el.append_child(&input).unwrap();
                    input.focus().unwrap();

                    let admin_save = admin_clone.clone();
                    let token_save = token_clone.clone();
                    let name_save = current_name.clone();
                    let email_save = current_email.clone();
                    let field_save = field_name.clone();
                    let make_save = |input_ref: web_sys::HtmlInputElement,
                                    admin_save: String,
                                    token_save: String,
                                    name_save: String,
                                    email_save: String,
                                    field_save: String,
                                    td_ref: Element| {
                        move || {
                            let new_val = input_ref.value();
                            let (new_name, new_email) = if field_save == "name" {
                                (new_val.clone(), email_save.clone())
                            } else {
                                (name_save.clone(), new_val.clone())
                            };
                            let admin = admin_save.clone();
                            let tok = token_save.clone();
                            let td = td_ref.clone();
                            let display = new_val.clone();
                            spawn_local(async move {
                                let client = api_client();
                                let Ok(admin_uuid) = admin.parse::<uuid::Uuid>() else { return; };
                                let Ok(user_uuid) = tok.parse::<uuid::Uuid>() else { return; };
                                let body = client::types::UpdateUserRequest {
                                    name: new_name,
                                    email: new_email,
                                    slot_id: None,
                                };
                                match client.update_user(&admin_uuid, &user_uuid, &body).await {
                                    Ok(_) => { td.set_text_content(Some(&display)); }
                                    Err(_) => { td.set_text_content(Some("Fehler")); }
                                }
                            });
                        }
                    };

                    // Save on Enter, revert on Escape
                    let save_enter = make_save(input.clone(), admin_save.clone(), token_save.clone(), name_save.clone(), email_save.clone(), field_save.clone(), td_el.clone());
                    let input_key = input.clone();
                    let td_revert = td_el.clone();
                    let revert_val = current.clone();
                    let onkeydown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::KeyboardEvent| {
                        match e.key().as_str() {
                            "Enter" => {
                                save_enter();
                                let _ = input_key.blur();
                            }
                            "Escape" => {
                                td_revert.set_text_content(Some(&revert_val));
                                let _ = input_key.blur();
                            }
                            _ => {}
                        }
                    });
                    input.add_event_listener_with_callback("keydown", onkeydown.as_ref().unchecked_ref()).unwrap();
                    onkeydown.forget();

                    // Save on blur (handles clicking away)
                    let save_blur = make_save(input.clone(), admin_save, token_save, name_save, email_save, field_save, td_el.clone());
                    let onblur = Closure::<dyn FnMut()>::new(move || { save_blur(); });
                    input.add_event_listener_with_callback("blur", onblur.as_ref().unchecked_ref()).unwrap();
                    onblur.forget();
                });
                td.add_event_listener_with_callback("dblclick", ondblclick.as_ref().unchecked_ref()).unwrap();
                ondblclick.forget();
                tr.append_child(&td).unwrap();
            }

            // Slot cell (double-click to change via dropdown)
            let td_slot = create_el(&doc, "td", &[("title", "Doppelklick zum Bearbeiten"), ("style", "cursor:pointer")]);
            td_slot.set_text_content(Some(slot_label));
            {
                let admin_clone = admin_uuid.clone();
                let token_clone = token.clone();
                let slots_clone = slots.clone();
                let current_name = user.name.clone();
                let current_email = user.email.clone();
                let td_el = td_slot.clone();
                let ondblclick = Closure::<dyn FnMut()>::new(move || {
                    let current_label = td_el.text_content().unwrap_or_default();
                    let select: web_sys::HtmlSelectElement = document()
                        .create_element("select").unwrap()
                        .dyn_into().unwrap();
                    select.set_class_name("field-input admin-inline-input");

                    // "—" option for no slot
                    let opt_none = document().create_element("option").unwrap();
                    opt_none.set_text_content(Some("—"));
                    opt_none.set_attribute("value", "").unwrap();
                    select.append_child(&opt_none).unwrap();

                    for slot in &slots_clone {
                        let opt = document().create_element("option").unwrap();
                        opt.set_text_content(Some(&slot.label));
                        opt.set_attribute("value", &slot.id.to_string()).unwrap();
                        if slot.label == current_label {
                            opt.set_attribute("selected", "selected").unwrap();
                        }
                        select.append_child(&opt).unwrap();
                    }

                    td_el.set_text_content(None);
                    td_el.append_child(&select).unwrap();
                    let _ = select.focus();

                    let admin_save = admin_clone.clone();
                    let token_save = token_clone.clone();
                    let name_save = current_name.clone();
                    let email_save = current_email.clone();
                    let td_ref = td_el.clone();
                    let slots_ref = slots_clone.clone();
                    let select_ref = select.clone();

                    let onchange = Closure::<dyn FnMut()>::new(move || {
                        let val = select_ref.value();
                        let slot_id: Option<i64> = if val.is_empty() { None } else { val.parse().ok() };
                        let display = slot_id
                            .and_then(|id| slots_ref.iter().find(|s| s.id == id))
                            .map(|s| s.label.clone())
                            .unwrap_or_else(|| "—".to_string());

                        let admin = admin_save.clone();
                        let tok = token_save.clone();
                        let name = name_save.clone();
                        let email = email_save.clone();
                        let td = td_ref.clone();

                        spawn_local(async move {
                            let client = api_client();
                            let Ok(admin_uuid) = admin.parse::<uuid::Uuid>() else { return; };
                            let Ok(user_uuid) = tok.parse::<uuid::Uuid>() else { return; };
                            let body = client::types::UpdateUserRequest {
                                name,
                                email,
                                slot_id,
                            };
                            match client.update_user(&admin_uuid, &user_uuid, &body).await {
                                Ok(_) => { td.set_text_content(Some(&display)); }
                                Err(_) => { td.set_text_content(Some("Fehler")); }
                            }
                        });
                    });
                    select.add_event_listener_with_callback("change", onchange.as_ref().unchecked_ref()).unwrap();
                    onchange.forget();
                });
                td_slot.add_event_listener_with_callback("dblclick", ondblclick.as_ref().unchecked_ref()).unwrap();
                ondblclick.forget();
            }
            tr.append_child(&td_slot).unwrap();

            let td_actions = create_el(&doc, "td", &[("class", "admin-actions")]);

            let open_href = token.parse::<uuid::Uuid>()
                .map(|u| format!("/{}", uuid_to_b64url(u)))
                .unwrap_or_else(|_| format!("/?uuid={}", token));
            let open_btn = create_el(&doc, "a", &[
                ("href", &open_href),
                ("class", "btn-icon"),
                ("target", "_blank"),
                ("title", "Öffnen"),
            ]);
            open_btn.set_text_content(Some("\u{f03cc}")); // nf-md-open_in_new
            td_actions.append_child(&open_btn).unwrap();

            // QR code button
            let qr_btn = create_el(&doc, "button", &[
                ("class", "btn-icon"),
                ("title", "QR-Code"),
            ]);
            qr_btn.set_text_content(Some("\u{f0432}")); // nf-md-qrcode
            let qr_url = format!("{}{}", origin(), open_href);
            let token_b64_qr = token_b64.clone();
            let onclick = Closure::<dyn FnMut()>::new(move || {
                show_qr_modal(&qr_url, &token_b64_qr);
            });
            qr_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
            onclick.forget();
            td_actions.append_child(&qr_btn).unwrap();

            let reset_btn = create_el(&doc, "button", &[
                ("class", "btn-icon"),
                ("title", "Buchung zurücksetzen"),
            ]);
            reset_btn.set_text_content(Some("\u{f0547}")); // nf-md-restart
            let admin_clone = admin_uuid.clone();
            let token_clone = token.clone();
            let onclick = Closure::<dyn FnMut()>::new(move || {
                let confirmed = window()
                    .unwrap()
                    .confirm_with_message("Buchung wirklich zurücksetzen?")
                    .unwrap_or(false);
                if !confirmed { return; }
                let admin_uuid = admin_clone.clone();
                let token = token_clone.clone();
                spawn_local(async move {
                    let client = api_client();
                    let Ok(admin) = admin_uuid.parse::<uuid::Uuid>() else { return; };
                    let Ok(user) = token.parse::<uuid::Uuid>() else { return; };
                    let _ = client.reset_booking(&admin, &user).await;
                    render_admin(admin_uuid).await;
                });
            });
            reset_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
            onclick.forget();
            td_actions.append_child(&reset_btn).unwrap();

            let delete_btn = create_el(&doc, "button", &[
                ("class", "btn-icon btn-icon--danger"),
                ("title", "Löschen"),
            ]);
            delete_btn.set_text_content(Some("\u{f0a7a}")); // nf-md-delete
            let admin_clone = admin_uuid.clone();
            let token_clone = token.clone();
            let onclick = Closure::<dyn FnMut()>::new(move || {
                let confirmed = window()
                    .unwrap()
                    .confirm_with_message("Benutzer wirklich löschen?")
                    .unwrap_or(false);
                if !confirmed { return; }
                let admin_uuid = admin_clone.clone();
                let token = token_clone.clone();
                spawn_local(async move {
                    let client = api_client();
                    let Ok(admin) = admin_uuid.parse::<uuid::Uuid>() else { return; };
                    let Ok(user) = token.parse::<uuid::Uuid>() else { return; };
                    let _ = client.delete_user(&admin, &user).await;
                    render_admin(admin_uuid).await;
                });
            });
            delete_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
            onclick.forget();
            td_actions.append_child(&delete_btn).unwrap();

            tr.append_child(&td_actions).unwrap();
            users_tbody.append_child(&tr).unwrap();
        }
    }

    // ── Add user button ───────────────────────────────────────────────────────
    let add_user_btn = doc.get_element_by_id("add-user-btn").unwrap();
    // Clone to replace the node and drop any previous listener
    let new_btn = add_user_btn.clone_node_with_deep(true).unwrap();
    add_user_btn.parent_node().unwrap().replace_child(&new_btn, &add_user_btn).unwrap();
    let admin_clone = admin_uuid.clone();
    let onclick = Closure::<dyn FnMut()>::new(move || {
        let admin_uuid = admin_clone.clone();
        spawn_local(async move {
            let client = api_client();
            let Ok(uuid) = admin_uuid.parse::<uuid::Uuid>() else { return; };
            match client.create_user(&uuid).await {
                Ok(_) => render_admin(admin_uuid).await,
                Err(_) => {},
            }
        });
    });
    new_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
    onclick.forget();

    // ── Download CSV button ───────────────────────────────────────────────────
    if let Some(csv_btn) = doc.get_element_by_id("download-csv-btn") {
        let new_csv_btn = csv_btn.clone_node_with_deep(true).unwrap();
        csv_btn.parent_node().unwrap().replace_child(&new_csv_btn, &csv_btn).unwrap();
        let users_clone = users.clone();
        let onclick = Closure::<dyn FnMut()>::new(move || {
            let mut csv = "Token,Name,E-Mail,Zeitfenster\n".to_string();
            for u in &users_clone {
                let token_b64 = uuid_to_b64url(u.token);
                let slot = u.slot_label.as_deref().unwrap_or("");
                csv.push_str(&format!(
                    "{},{},{},{}\n",
                    token_b64,
                    csv_escape(&u.name),
                    csv_escape(&u.email),
                    csv_escape(slot),
                ));
            }
            download_text(&csv, "benutzer.csv", "text/csv;charset=utf-8");
        });
        new_csv_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
        onclick.forget();
    }

    // ── Slots tbody ───────────────────────────────────────────────────────────
    let slots_tbody = doc.get_element_by_id("slots-tbody").unwrap();
    slots_tbody.set_inner_html("");

    if slots.is_empty() {
        let tr = create_el(&doc, "tr", &[]);
        let td = create_el(&doc, "td", &[("colspan", "4"), ("class", "admin-empty")]);
        td.set_text_content(Some("Keine Slots vorhanden."));
        tr.append_child(&td).unwrap();
        slots_tbody.append_child(&tr).unwrap();
    } else {
        for slot in &slots {
            let tr = create_el(&doc, "tr", &[]);
            for val in [
                slot.label.as_str(),
                &slot.max_bookings.to_string(),
                &slot.current_bookings.to_string(),
            ] {
                let td = create_el(&doc, "td", &[]);
                td.set_text_content(Some(val));
                tr.append_child(&td).unwrap();
            }

            let td_actions = create_el(&doc, "td", &[("class", "admin-actions")]);
            let del_btn = create_el(&doc, "button", &[
                ("class", "btn-icon btn-icon--danger"),
                ("title", "Löschen"),
            ]);
            del_btn.set_text_content(Some("\u{f0a7a}")); // nf-md-delete

            let admin_clone = admin_uuid.clone();
            let slot_id = slot.id;
            let onclick = Closure::<dyn FnMut()>::new(move || {
                let confirmed = window()
                    .unwrap()
                    .confirm_with_message("Slot wirklich löschen?")
                    .unwrap_or(false);
                if !confirmed { return; }
                let admin_uuid = admin_clone.clone();
                spawn_local(async move {
                    let client = api_client();
                    let Ok(admin) = admin_uuid.parse::<uuid::Uuid>() else { return; };
                    let _ = client.delete_slot(&admin, slot_id).await;
                    render_admin(admin_uuid).await;
                });
            });
            del_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
            onclick.forget();

            td_actions.append_child(&del_btn).unwrap();
            tr.append_child(&td_actions).unwrap();
            slots_tbody.append_child(&tr).unwrap();
        }
    }

}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn create_el(doc: &Document, tag: &str, attrs: &[(&str, &str)]) -> Element {
    let el = doc.create_element(tag).unwrap();
    for (k, v) in attrs {
        el.set_attribute(k, v).unwrap();
    }
    el
}


fn input_value(doc: &Document, id: &str) -> String {
    doc.get_element_by_id(id)
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

fn set_error(msg: &str) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id("form-error") {
        el.set_text_content(Some(msg));
        el.set_class_name("form-error");
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn download_text(content: &str, filename: &str, mime: &str) {
    let doc = document();
    let a: web_sys::HtmlAnchorElement = doc.create_element("a").unwrap().unchecked_into();
    let encoded = js_sys::encode_uri_component(content);
    a.set_href(&format!("data:{},{}", mime, encoded));
    a.set_download(filename);
    doc.body().unwrap().append_child(&a).unwrap();
    a.click();
    a.remove();
}

fn qr_svg(url: &str) -> String {
    let code = QrCode::with_error_correction_level(url, EcLevel::M).unwrap();
    let colors = code.render::<qrcode::render::svg::Color>()
        .quiet_zone(true)
        .build();
    colors
}

fn svg_to_png_download(svg: &str, filename: &str) {
    let doc = document();
    let size = 512u32;

    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(svg));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("image/svg+xml");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).unwrap();
    let svg_url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();

    let img: web_sys::HtmlImageElement = doc.create_element("img").unwrap().unchecked_into();
    let canvas: web_sys::HtmlCanvasElement = doc.create_element("canvas").unwrap().unchecked_into();
    canvas.set_width(size);
    canvas.set_height(size);

    let filename = filename.to_string();
    let svg_url_clone = svg_url.clone();
    let img_clone = img.clone();

    let onload = Closure::<dyn FnMut()>::new(move || {
        let ctx: web_sys::CanvasRenderingContext2d = canvas
            .get_context("2d").unwrap().unwrap().unchecked_into();
        ctx.set_fill_style_str("#ffffff");
        ctx.fill_rect(0.0, 0.0, size as f64, size as f64);
        ctx.draw_image_with_html_image_element_and_dw_and_dh(
            &img_clone, 0.0, 0.0, size as f64, size as f64,
        ).unwrap();
        web_sys::Url::revoke_object_url(&svg_url_clone).unwrap();

        let data_url = canvas.to_data_url_with_type("image/png").unwrap();
        let a: web_sys::HtmlAnchorElement = document()
            .create_element("a").unwrap().unchecked_into();
        a.set_href(&data_url);
        a.set_download(&filename);
        document().body().unwrap().append_child(&a).unwrap();
        a.click();
        a.remove();
    });
    img.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
    img.set_src(&svg_url);
}

fn show_qr_modal(url: &str, token_b64: &str) {
    let doc = document();
    let svg = qr_svg(url);
    let filename = format!("qrcode-{}.png", token_b64);

    let overlay = create_el(&doc, "div", &[("id", "qr-modal"), ("class", "qr-modal")]);
    let box_el = create_el(&doc, "div", &[("class", "qr-modal-box")]);

    let svg_wrap = create_el(&doc, "div", &[("class", "qr-svg")]);
    svg_wrap.set_inner_html(&svg);
    box_el.append_child(&svg_wrap).unwrap();

    let url_p = create_el(&doc, "p", &[("class", "qr-url")]);
    url_p.set_text_content(Some(url));
    box_el.append_child(&url_p).unwrap();

    let btn_row = create_el(&doc, "div", &[("class", "qr-btn-row")]);

    let dl_btn = create_el(&doc, "button", &[("class", "btn btn-primary")]);
    dl_btn.set_text_content(Some("PNG herunterladen"));
    let svg_clone = svg.clone();
    let filename_clone = filename.clone();
    let dl_onclick = Closure::<dyn FnMut()>::new(move || {
        svg_to_png_download(&svg_clone, &filename_clone);
    });
    dl_btn.add_event_listener_with_callback("click", dl_onclick.as_ref().unchecked_ref()).unwrap();
    dl_onclick.forget();
    btn_row.append_child(&dl_btn).unwrap();

    let close_btn = create_el(&doc, "button", &[("class", "btn btn-secondary")]);
    close_btn.set_text_content(Some("Schließen"));
    btn_row.append_child(&close_btn).unwrap();

    box_el.append_child(&btn_row).unwrap();
    overlay.append_child(&box_el).unwrap();
    doc.body().unwrap().append_child(&overlay).unwrap();

    let onclick = Closure::<dyn FnMut()>::new(move || {
        if let Some(el) = document().get_element_by_id("qr-modal") {
            el.remove();
        }
    });
    close_btn.add_event_listener_with_callback("click", onclick.as_ref().unchecked_ref()).unwrap();
    onclick.forget();

    // Also close on overlay click outside the box
    let onclick2 = Closure::<dyn FnMut(_)>::new(|e: web_sys::MouseEvent| {
        let target = e.target().unwrap();
        let el: Element = target.unchecked_into();
        if el.id() == "qr-modal" {
            el.remove();
        }
    });
    overlay.add_event_listener_with_callback("click", onclick2.as_ref().unchecked_ref()).unwrap();
    onclick2.forget();
}

fn show_invalid_code() {
    let doc = document();
    if let Some(el) = doc.query_selector(".invalid-code").ok().flatten() {
        el.set_class_name("invalid-code visible");
    }
}

fn show_error(msg: &str) {
    let doc = document();
    if let Some(body) = doc.body() {
        let div = create_el(&doc, "div", &[("class", "container")]);
        let p = create_el(&doc, "p", &[("class", "error-banner")]);
        p.set_text_content(Some(msg));
        div.append_child(&p).unwrap();
        body.set_inner_html("");
        body.append_child(&div).unwrap();
    }
}
