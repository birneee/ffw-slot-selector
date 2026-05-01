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

// Read ?uuid=... from query string
fn parse_uuid_param() -> Option<String> {
    let search = window().unwrap().location().search().unwrap_or_default();
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            if k == "uuid" { Some(v.to_string()) } else { None }
        })
}

// Detect view vs edit from pathname
fn parse_path() -> Option<(String, bool)> {
    let uuid = parse_uuid_param()?;
    let is_edit = pathname().trim_end_matches('/') == "/edit";
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
        return;
    };
    let Ok(uuid) = uuid_str.parse::<uuid::Uuid>() else {
        show_error("Dieser Link ist ungültig oder wurde bereits gelöscht.");
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
            mount_view(&uuid.to_string(), &data);
        }
        Err(_) => show_error("Dieser Link ist ungültig oder wurde bereits gelöscht."),
    }
}

fn mount_view(uuid: &str, data: &BookingPageResponse) {
    let doc = document();

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
        .map(|id| {
            data.slots
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.label.as_str())
                .unwrap_or("Unbekannt")
                .to_string()
        })
        .unwrap_or_else(|| "Kein Slot gewählt".to_string());
    if let Some(el) = doc.get_element_by_id("view-slot") {
        el.set_text_content(Some(&slot_val));
        el.set_class_name("info-value");
    }

    let btn_label = if data.user_name.is_empty() && data.current_slot_id.is_none() {
        "Slot buchen"
    } else {
        "Bearbeiten"
    };
    let edit_href = format!("/edit?uuid={}", uuid);
    if let Some(btn) = doc.get_element_by_id("view-edit-btn") {
        // Replace node to drop the loading-guard click listener
        let new_btn = btn.clone_node_with_deep(true).unwrap();
        btn.parent_node().unwrap().replace_child(&new_btn, &btn).unwrap();
        let new_btn: Element = new_btn.unchecked_into();
        new_btn.set_text_content(Some(btn_label));
        new_btn.set_attribute("href", &edit_href).unwrap();
    }
}

// ── Edit page ────────────────────────────────────────────────────────────────

async fn render_edit(uuid: uuid::Uuid) {
    let client = api_client();
    match client.get_booking_page(&uuid).await {
        Ok(resp) => {
            let data = resp.into_inner();
            mount_edit(uuid, &data);
        }
        Err(_) => show_error("Dieser Link ist ungültig oder wurde bereits gelöscht."),
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
                    window().unwrap().location().set_href(&format!("/?uuid={}", uuid_str)).unwrap();
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
        let td = create_el(&doc, "td", &[("colspan", "4"), ("class", "admin-empty")]);
        td.set_text_content(Some("Keine Benutzer vorhanden."));
        tr.append_child(&td).unwrap();
        users_tbody.append_child(&tr).unwrap();
    } else {
        for user in &users {
            let tr = create_el(&doc, "tr", &[]);
            let token = user.token.to_string();
            for val in [token.as_str(), user.name.as_str(), user.email.as_str()] {
                let td = create_el(&doc, "td", &[]);
                td.set_text_content(Some(val));
                tr.append_child(&td).unwrap();
            }

            let td_actions = create_el(&doc, "td", &[("class", "admin-actions")]);

            let open_href = format!("/?uuid={}", token);
            let open_btn = create_el(&doc, "a", &[
                ("href", &open_href),
                ("class", "btn-icon"),
                ("target", "_blank"),
                ("title", "Öffnen"),
            ]);
            open_btn.set_text_content(Some("\u{f03cc}")); // nf-md-open_in_new
            td_actions.append_child(&open_btn).unwrap();

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
