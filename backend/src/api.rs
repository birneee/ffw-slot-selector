use async_trait::async_trait;
use axum_extra::extract::CookieJar;
use headers::Host;
use http::Method;

use crate::apis::{
    default::{
        BookSlotResponse, CreateSlotResponse, CreateUserResponse, DeleteSlotResponse,
        DeleteUserResponse, GetBookingPageResponse, ListSlotsResponse, ListUsersResponse,
        ResetBookingResponse, UpdateUserResponse,
    },
    ErrorHandler,
};
use crate::db::{self, BookResult};
use crate::email::EmailConfig;
use crate::models;

#[derive(Clone)]
pub struct Api {
    pub pool: sqlx::SqlitePool,
    pub email_config: Option<EmailConfig>,
}

impl AsRef<Api> for Api {
    fn as_ref(&self) -> &Api {
        self
    }
}

impl ErrorHandler<()> for Api {}

#[async_trait]
impl crate::apis::default::Default<()> for Api {
    async fn get_booking_page(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::GetBookingPagePathParams,
    ) -> Result<GetBookingPageResponse, ()> {
        let user = match db::get_user_by_uuid(&self.pool, path_params.user_uuid).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                return Ok(GetBookingPageResponse::Status404_UUIDNotFound(
                    models::ErrorResponse::new("unknown token".into()),
                ))
            }
            Err(e) => {
                tracing::error!("db error: {e}");
                return Err(());
            }
        };

        let slots = match db::get_slots(&self.pool).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("db error: {e}");
                return Err(());
            }
        };

        let current_slot_id = match db::get_current_slot_id(&self.pool, &user.uuid).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("db error: {e}");
                return Err(());
            }
        };

        let mut resp = models::BookingPageResponse::new(
            user.name,
            user.email,
            slots
                .into_iter()
                .map(|s| models::Slot::new(s.id, s.label, s.max_bookings as i32, s.current_bookings as i32))
                .collect(),
        );
        resp.current_slot_id = current_slot_id.map(crate::types::Nullable::Present);

        Ok(GetBookingPageResponse::Status200_UserInfoAndAvailableSlots(resp))
    }

    async fn book_slot(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::BookSlotPathParams,
        body: &models::BookSlotRequest,
    ) -> Result<BookSlotResponse, ()> {
        let user = match db::get_user_by_uuid(&self.pool, path_params.user_uuid).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                return Ok(BookSlotResponse::Status404_UUIDNotFound(
                    models::ErrorResponse::new("unknown token".into()),
                ))
            }
            Err(e) => {
                tracing::error!("db error: {e}");
                return Err(());
            }
        };

        if body.name.trim().is_empty() || body.name.len() > 200 {
            return Ok(BookSlotResponse::Status409_SlotIsFull(
                models::ErrorResponse::new("invalid name".into()),
            ));
        }
        if body.email.trim().is_empty() || body.email.len() > 200 || !body.email.contains('@') {
            return Ok(BookSlotResponse::Status409_SlotIsFull(
                models::ErrorResponse::new("invalid email".into()),
            ));
        }

        match db::book_slot(&self.pool, &user.uuid, &body.name, &body.email, body.slot_id).await {
            Ok(BookResult::Ok { slot_id, label }) => {
                if let Some(cfg) = &self.email_config {
                    if let Err(e) = crate::email::send_confirmation(cfg, &body.email, &body.name, &label).await {
                        tracing::warn!("failed to send confirmation email to {}: {e}", body.email);
                    }
                }
                Ok(BookSlotResponse::Status200_BookingConfirmed(
                    models::BookSlotResponse::new(slot_id, label),
                ))
            }
            Ok(BookResult::SlotFull) => Ok(BookSlotResponse::Status409_SlotIsFull(
                models::ErrorResponse::new("slot is full".into()),
            )),
            Ok(BookResult::AlreadyBooked) => Ok(BookSlotResponse::Status409_SlotIsFull(
                models::ErrorResponse::new("already booked".into()),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn update_user(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::UpdateUserPathParams,
        body: &models::UpdateUserRequest,
    ) -> Result<UpdateUserResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(UpdateUserResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        if body.name.trim().is_empty() || body.name.len() > 200 {
            return Ok(UpdateUserResponse::Status403_Unauthorized(
                models::ErrorResponse::new("invalid name".into()),
            ));
        }
        if body.email.trim().is_empty() || body.email.len() > 200 || !body.email.contains('@') {
            return Ok(UpdateUserResponse::Status403_Unauthorized(
                models::ErrorResponse::new("invalid email".into()),
            ));
        }

        let user_uuid = path_params.user_uuid.to_string();
        let slot_id = body.slot_id.as_ref().and_then(|n| match n {
            crate::types::Nullable::Present(v) => Some(*v),
            crate::types::Nullable::Null => None,
        });
        match db::update_user(&self.pool, &user_uuid, &body.name, &body.email, slot_id).await {
            Ok(true) => {
                let mut user = models::User::new(path_params.user_uuid, body.name.clone(), body.email.clone());
                user.slot_label = None;
                Ok(UpdateUserResponse::Status200_UserUpdated(user))
            }
            Ok(false) => Ok(UpdateUserResponse::Status404_UserNotFound(
                models::ErrorResponse::new("user not found".into()),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn reset_booking(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::ResetBookingPathParams,
    ) -> Result<ResetBookingResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(ResetBookingResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }
        let user_uuid = path_params.user_uuid.to_string();
        match db::reset_booking(&self.pool, &user_uuid).await {
            Ok(true) => Ok(ResetBookingResponse::Status204_BookingCleared),
            Ok(false) => Ok(ResetBookingResponse::Status404_UserNotFound(
                models::ErrorResponse::new("user not found".into()),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn delete_user(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::DeleteUserPathParams,
    ) -> Result<DeleteUserResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(DeleteUserResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        let user_uuid = path_params.user_uuid.to_string();
        match db::delete_user(&self.pool, &user_uuid).await {
            Ok(true) => Ok(DeleteUserResponse::Status204_UserDeleted),
            Ok(false) => Ok(DeleteUserResponse::Status404_UserNotFound(
                models::ErrorResponse::new("user not found".into()),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn list_users(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::ListUsersPathParams,
    ) -> Result<ListUsersResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(ListUsersResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        match db::list_users(&self.pool).await {
            Ok(users) => Ok(ListUsersResponse::Status200_ListOfUsers(
                users
                    .into_iter()
                    .map(|u| {
                        let mut user = models::User::new(u.token.parse().unwrap(), u.name, u.email);
                        user.slot_label = u.slot_label.map(crate::types::Nullable::Present);
                        user
                    })
                    .collect(),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn list_slots(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::ListSlotsPathParams,
    ) -> Result<ListSlotsResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(ListSlotsResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        match db::list_slots_admin(&self.pool).await {
            Ok(slots) => Ok(ListSlotsResponse::Status200_ListOfSlots(
                slots
                    .into_iter()
                    .map(|s| models::Slot::new(s.id, s.label, s.max_bookings as i32, s.current_bookings as i32))
                    .collect(),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn create_slot(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::CreateSlotPathParams,
        body: &models::CreateSlotRequest,
    ) -> Result<CreateSlotResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(CreateSlotResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        if body.label.trim().is_empty() || body.label.len() > 100 {
            return Ok(CreateSlotResponse::Status403_Unauthorized(
                models::ErrorResponse::new("invalid label".into()),
            ));
        }
        if body.max_bookings < 1 || body.max_bookings > 10_000 {
            return Ok(CreateSlotResponse::Status403_Unauthorized(
                models::ErrorResponse::new("invalid max_bookings".into()),
            ));
        }

        match db::create_slot(&self.pool, &body.label, body.max_bookings).await {
            Ok(s) => Ok(CreateSlotResponse::Status200_CreatedSlot(
                models::Slot::new(s.id, s.label, s.max_bookings as i32, s.current_bookings as i32),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn delete_slot(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::DeleteSlotPathParams,
    ) -> Result<DeleteSlotResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(DeleteSlotResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        match db::delete_slot(&self.pool, path_params.slot_id).await {
            Ok(true) => Ok(DeleteSlotResponse::Status204_SlotDeleted),
            Ok(false) => Ok(DeleteSlotResponse::Status404_SlotNotFound(
                models::ErrorResponse::new("slot not found".into()),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }

    async fn create_user(
        &self,
        _method: &Method,
        _host: &Host,
        _cookies: &CookieJar,
        path_params: &models::CreateUserPathParams,
    ) -> Result<CreateUserResponse, ()> {
        let admin_uuid = path_params.admin_uuid.to_string();
        match db::is_valid_admin(&self.pool, &admin_uuid).await {
            Ok(false) | Err(_) => {
                return Ok(CreateUserResponse::Status403_Unauthorized(
                    models::ErrorResponse::new("unauthorized".into()),
                ))
            }
            _ => {}
        }

        match db::create_user(&self.pool).await {
            Ok(u) => Ok(CreateUserResponse::Status200_CreatedUser(
                models::User::new(u.token.parse().unwrap(), u.name, u.email),
            )),
            Err(e) => {
                tracing::error!("db error: {e}");
                Err(())
            }
        }
    }
}
