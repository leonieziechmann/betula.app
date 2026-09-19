#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use spike_ssr::app::{shell, App};

    let mut args = std::env::args().skip(1);
    if let Some(snapshot) = args.next() { std::env::set_var("BTU_SNAPSHOT", snapshot); }
    let site_root = args.next().unwrap_or_else(|| "site".to_string());
    let addr: std::net::SocketAddr = "127.0.0.1:8793".parse().unwrap();
    let options = LeptosOptions::builder()
        .output_name("spike")
        .site_root(site_root)
        .site_pkg_dir("pkg")
        .site_addr(addr)
        .build();
    let routes = generate_route_list(App);
    let app = Router::new()
        .leptos_routes(&options, routes, {
            let options = options.clone();
            move || shell(options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(options);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

#[cfg(not(feature = "ssr"))]
fn main() {}
