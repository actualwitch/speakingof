
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::env;

    use axum::Router;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list_with_ssg, LeptosRoutes};
    use speakingof::app::*;

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    // Generate the list of routes in your Leptos App
    let (routes, static_routes) = generate_route_list_with_ssg({
        let leptos_options = conf.leptos_options.clone();
        move || shell(leptos_options.to_owned())
    });

    static_routes.generate(&conf.leptos_options).await;

    let leptos_options = conf.leptos_options.clone();

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let copy = leptos_options.clone();
            move || shell(copy.to_owned())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(conf.leptos_options);

    match env::var("PRERENDER_ONLY") {
        Ok(_) => {},
        _ => {
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        }
    }
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
