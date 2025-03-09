//! Run with:
//!
//! ```sh
//! dx build --features web
//! cargo run --features server
//! ```

#![allow(non_snake_case, unused)]

#[cfg(feature = "server")]
mod auth;

use dioxus::prelude::*;
use dioxus_fullstack::prelude::*;
use serde::{Deserialize, Serialize};

fn main() {
    // Set the logger ahead of time since we don't use `dioxus::launch` on the server
    dioxus::logger::initialize_default();

    #[cfg(feature = "web")]
    // Hydrate the application on the client
    dioxus_web::launch::launch_cfg(App, dioxus_web::Config::new().hydrate(true));

    #[cfg(feature = "server")]
    {
        use crate::auth::*;
        use axum::routing::*;
        use axum_session::SessionConfig;
        use axum_session::SessionStore;
        use axum_session_auth::AuthConfig;
        use axum_session_auth::SessionSqlitePool;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let pool = connect_to_database().await;

                //This Defaults as normal Cookies.
                //To enable Private cookies for integrity, and authenticity please check the next Example.
                let session_config = SessionConfig::default().with_table_name("test_table");
                let auth_config = AuthConfig::<i64>::default().with_anonymous_user_id(Some(1));
                let session_store = SessionStore::<SessionSqlitePool>::new(
                    Some(pool.clone().into()),
                    session_config,
                )
                .await
                .unwrap();

                User::create_user_tables(&pool).await;

                // build our application with some routes
                let app = Router::new()
                    // Server side render the application, serve static assets, and register server functions
                    .serve_dioxus_application(ServeConfig::new().unwrap(), App)
                    .layer(
                        axum_session_auth::AuthSessionLayer::<
                            crate::auth::User,
                            i64,
                            axum_session_auth::SessionSqlitePool,
                            sqlx::SqlitePool,
                        >::new(Some(pool))
                        .with_config(auth_config),
                    )
                    .layer(axum_session::SessionLayer::new(session_store));

                // run it
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 41588));
                let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

                axum::serve(listener, app.into_make_service())
                    .await
                    .unwrap();
            });
    }
}

fn App() -> Element {
    let mut user_name = use_signal(|| "?".to_string());
    let mut permissions = use_signal(|| "?".to_string());
    rsx! {
        // The Stylesheet component inserts a style link into the head of the document
        document::Stylesheet {
            // Urls are relative to your Cargo.toml file
            href: asset!("/assets/tailwind.css"),
        }
        h1 { class: "text-3xl font-bold", "Axum Session Auth Example" }
        div {
            button {
                onclick: move |_| {
                    async move {
                        login().await.unwrap();
                    }
                },
                "Login Test User"
            }
        }
        div {
            button {
                onclick: move |_| async move {
                    if let Ok(data) = get_user_name().await {
                        user_name.set(data);
                    }
                },
                "Get User Name"
            }
            "User name: {user_name}"
        }
        div {
            button {
                onclick: move |_| async move {
                    if let Ok(data) = get_permissions().await {
                        permissions.set(data);
                    }
                },
                "Get Permissions"
            }
            "Permissions: {permissions}"
        }
    }
}

#[server(GetUserName)]
pub async fn get_user_name() -> Result<String, ServerFnError> {
    let session: crate::auth::Session = extract().await?;
    Ok(session.0.current_user.unwrap().username.to_string())
}

#[server(Login)]
pub async fn login() -> Result<(), ServerFnError> {
    let auth: crate::auth::Session = extract().await?;
    auth.login_user(2);
    Ok(())
}

#[server(Permissions)]
pub async fn get_permissions() -> Result<String, ServerFnError> {
    let method: axum::http::Method = extract().await?;
    let auth: crate::auth::Session = extract().await?;
    let current_user = auth.current_user.clone().unwrap_or_default();

    // lets check permissions only and not worry about if they are anon or not
    if !axum_session_auth::Auth::<crate::auth::User, i64, sqlx::SqlitePool>::build(
        [axum::http::Method::POST],
        false,
    )
    .requires(axum_session_auth::Rights::any([
        axum_session_auth::Rights::permission("Category::View"),
        axum_session_auth::Rights::permission("Admin::View"),
    ]))
    .validate(&current_user, &method, None)
    .await
    {
        return Ok(format!(
            "User {}, Does not have permissions needed to view this page please login",
            current_user.username
        ));
    }

    Ok(format!(
        "User has Permissions needed. Here are the Users permissions: {:?}",
        current_user.permissions
    ))
}
