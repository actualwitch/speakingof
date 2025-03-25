use chrono::prelude::*;
use comrak::{
    format_html_with_plugins,
    nodes::{AstNode, NodeHeading, NodeLink, NodeValue},
    parse_document,
    plugins::syntect::SyntectAdapterBuilder,
    Arena, ExtensionOptions, Options, Plugins,
};
use futures::{channel::mpsc, Stream};
use leptos::{logging::log, prelude::*};
use leptos_meta::MetaTags;
use leptos_meta::*;
use leptos_router::{
    components::{FlatRoutes, Route, Router, A},
    hooks::use_params,
    params::Params,
    path,
    static_routes::StaticRoute,
    SsrMode,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

use crate::constants::{ICON, TITLE, TRIANGLE, BASE_URL};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <MetaTags/>
                <style>
                    {include_str!("style/reset.css")}
                </style>
                <style>
                    {include_str!("style/tods.css")}
                </style>
                <style>
                    {include_str!("style/app.css")}
                </style>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();
    let fallback = || view! { "Page not found." }.into_view();

    view! {
        <Meta name="color-scheme" content="dark light"/>
        <Link rel="icon" href=format!("data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>{ICON}</text></svg>") />
        <Router>
            <main class="prose">
                <FlatRoutes fallback>
                    <Route
                        path=path!("/")
                        view=HomePage
                        ssr=SsrMode::Static(
                            StaticRoute::new().regenerate(|_| watch_path(Path::new("./articles"))),
                        )
                    />
                    <Route
                        path=path!("/:slug/")
                        view=Post
                        ssr=SsrMode::Static(
                            StaticRoute::new()
                                .prerender_params(|| async move {
                                    [("slug".into(), list_slugs().await.unwrap_or_default())]
                                        .into_iter()
                                        .collect()
                                })
                                .regenerate(|params| {
                                    let slug = params.get("slug").unwrap();
                                    watch_path(Path::new(&format!("./articles/{slug}.md")))
                                }),
                        )
                    />

                </FlatRoutes>
            </main>
        </Router>
    }
}

#[component]
fn NavBar(#[prop(optional)] show_back: Option<bool>) -> impl IntoView {
    view! {
        <h1>
            {show_back.and_then(|show| {
                if show {
                    Some(view!{<a href="/">"⏎"</a>" "})
                } else {
                    None
                }
            })}
            "Speaking of "
            <A href="https://www.actualwitch.me">Witch</A>
        </h1>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    // load the posts
    let posts = Resource::new(|| (), |_| list_posts());
    let posts = move || {
        posts
            .get()
            .map(|n| n.unwrap_or_default())
            .unwrap_or_default()
    };

    view! {
        <Title text=TITLE />
        <NavBar />
        <Suspense fallback=move || view! { <p>"Loading posts..."</p> }>
            <ul class="semantic">
                <For each=posts key=|post| post.slug.clone() let:post>
                    <li>
                        <a href=format!("/{}/", post.slug)>{post.title.clone()}</a>
                        <p>{post.description}</p>
                    </li>
                </For>
            </ul>
        </Suspense>
    }
}

#[derive(Params, Clone, Debug, PartialEq, Eq)]
pub struct PostParams {
    slug: Option<String>,
}

#[component]
fn Post() -> impl IntoView {
    let query = use_params::<PostParams>();
    let slug = move || {
        query
            .get()
            .map(|q| q.slug.unwrap_or_default())
            .map_err(|_| PostError::InvalidId)
    };
    let post_resource = Resource::new_blocking(slug, |slug| async move {
        match slug {
            Err(e) => Err(e),
            Ok(slug) => get_post(slug)
                .await
                .map(|data| data.ok_or(PostError::PostNotFound))
                .map_err(|e| PostError::ServerError(e.to_string())),
        }
    });

    let post_view = move || {
        Suspend::new(async move {
            match post_resource.await {
                Ok(Ok(post)) => {
                    Ok(view! {
                        <NavBar show_back=true />
                        <h2>{post.title.clone()}</h2>
                        <p inner_html=post.content.clone() />

                        // meta + opengraph
                        <Title text=post.title.clone()/>
                        <Meta name="og:title" content=post.title/>
                        <Meta name="og:url" content=format!("{BASE_URL}/{}", post.slug)/>
                        {post.description.and_then(|desc| Some(view! {<Meta name="og:description" content=desc/>}))}
                        {post.image.and_then(|image| Some(view! {<Meta name="og:image" content=format!("{BASE_URL}{image}")/>}))}
                    })
                }
                Ok(Err(e)) | Err(e) => Err(PostError::ServerError(e.to_string())),
            }
        })
    };

    view! {
        <Suspense fallback=move || view! { <p>"Loading post..."</p> }>
            <ErrorBoundary fallback=|errors| {
                #[cfg(feature = "ssr")]
                expect_context::<leptos_axum::ResponseOptions>()
                    .set_status(http::StatusCode::NOT_FOUND);
                view! {
                    <div class="error">
                        <h1>"Something went wrong."</h1>
                        <ul>
                            {move || {
                                errors
                                    .get()
                                    .into_iter()
                                    .map(|(_, error)| view! { <li>{error.to_string()}</li> })
                                    .collect::<Vec<_>>()
                            }}

                        </ul>
                    </div>
                }
            }>{post_view}</ErrorBoundary>
        </Suspense>
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostError {
    #[error("Invalid post ID.")]
    InvalidId,
    #[error("Post not found.")]
    PostNotFound,
    #[error("Server error: {0}.")]
    ServerError(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    slug: String,
    title: String,
    content: String,
    description: Option<String>,
    image: Option<String>,
    created_at: DateTime<FixedOffset>,
    modified_at: DateTime<FixedOffset>,
}

#[server]
pub async fn list_slugs() -> Result<Vec<String>, ServerFnError> {
    use tokio::fs;
    use tokio_stream::wrappers::ReadDirStream;
    use tokio_stream::StreamExt;

    let files = ReadDirStream::new(fs::read_dir("./articles").await?);
    Ok(files
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let extension = path.extension()?;
            if extension != "md" {
                return None;
            }

            let slug = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .replace(".md", "");

            if slug.starts_with("draft-") {
                return None;
            }
            Some(slug)
        })
        .collect()
        .await)
}

pub fn parse_markdown(text: String) -> (String, String, Option<String>, Option<String>) {
    let arena = Arena::new();

    let extension = ExtensionOptions::builder()
        .alerts(true)
        .table(true)
        .underline(true)
        .front_matter_delimiter(TRIANGLE.to_string())
        .build();

    let options = Options {
        extension,
        ..Options::default()
    };

    let syntect = SyntectAdapterBuilder::new()
        .theme("base16-ocean.light")
        .build();
    let mut plugins = Plugins::default();

    plugins.render.codefence_syntax_highlighter = Some(&syntect);

    let root = parse_document(&arena, &text, &options);
    let mut html = vec![];
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut image: Option<String> = None;

    for node in root.children() {
        let mut node_value = node.data.borrow_mut();
        
        match &node_value.value {
            NodeValue::FrontMatter(fm) => {
                if description.is_none() {
                    description = Some(fm.replace(TRIANGLE, ""));
                }
                drop(node_value);
                continue;
            }
            NodeValue::Heading(NodeHeading { level: 1, .. }) => {
                drop(node_value);
                title = node
                    .first_child()
                    .unwrap()
                    .data
                    .borrow()
                    .value
                    .clone()
                    .text()
                    .cloned();
                continue;
            }
            _ => {}
        }

        if let NodeValue::Heading(heading) = &mut node_value.value {
            if heading.level != 1 {
                heading.level = 3;
            }
        }

        drop(node_value);
        format_html_with_plugins(node, &options, &mut html, &plugins).unwrap();
    }

    if image.is_none() {
        image = find_first_image(root);
    }

    (
        String::from_utf8(html).unwrap(),
        title.unwrap_or_default(),
        description,
        image,
    )
}


fn find_first_image<'a>(node: &'a AstNode<'a>) -> Option<String> {
    use comrak::nodes::AstNode;
    
    for child in node.children() {
        let data = child.data.borrow();
        // If node is an image, return it immediately
        if let NodeValue::Image(NodeLink { url, .. }) = &data.value {
            return Some(url.to_string());
        }
        drop(data);
        // Otherwise, keep walking children
        if let Some(img_url) = find_first_image(child) {
            return Some(img_url);
        }
    }
    None
}

#[server]
pub async fn list_posts() -> Result<Vec<Post>, ServerFnError> {
    println!("calling list_posts");

    use futures::TryStreamExt;
    use tokio::fs;
    use tokio_stream::wrappers::ReadDirStream;

    let files = ReadDirStream::new(fs::read_dir("./articles").await?);
    files
        .try_filter_map(|entry| async move {
            let path = entry.path();

            if !path.is_file() {
                return Ok(None);
            }
            let Some(extension) = path.extension() else {
                return Ok(None);
            };
            if extension != "md" {
                return Ok(None);
            }

            let slug = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .replace(".md", "");

            if slug.starts_with("draft-") {
                return Ok(None);
            }

            let meta = path.metadata().expect("should have metadata");
            let created_at: DateTime<Local> =
                meta.created().expect("should have creation date").into();
            let modified_at: DateTime<Local> =
                meta.modified().expect("should have modified date").into();
            let (content, title, description, image) =
                parse_markdown(fs::read_to_string(path).await?);

            Ok(Some(Post {
                slug,
                title,
                content,
                description,
                image,
                created_at: created_at.into(),
                modified_at: modified_at.into(),
            }))
        })
        .try_collect()
        .await
        .map_err(ServerFnError::from)
}

#[server]
pub async fn get_post(slug: String) -> Result<Option<Post>, ServerFnError> {
    let path = format!("./articles/{slug}.md");

    let meta = tokio::fs::metadata(&path).await?;
    let created_at: DateTime<Local> = meta.created().expect("should have creation date").into();
    let modified_at: DateTime<Local> = meta.modified().expect("should have modified date").into();

    let (content, title, description, image) =
        parse_markdown(tokio::fs::read_to_string(&path).await?);
    Ok(Some(Post {
        slug,
        title,
        content,
        description,
        image,
        created_at: created_at.into(),
        modified_at: modified_at.into(),
    }))
}

#[allow(unused)] // path is not used in non-SSR
fn watch_path(path: &Path) -> impl Stream<Item = ()> {
    #[allow(unused)]
    let (mut tx, rx) = mpsc::channel(0);

    #[cfg(feature = "ssr")]
    {
        use notify::RecursiveMode;
        use notify::Watcher;

        let mut watcher = notify::recommended_watcher(move |res: Result<_, _>| {
            if res.is_ok() {
                // if this fails, it's because the buffer is full
                // this means we've already notified before it's regenerated,
                // so this page will be queued for regeneration already
                _ = tx.try_send(());
            }
        })
        .expect("could not create watcher");

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .expect("could not watch path");

        // we want this to run as long as the server is alive
        std::mem::forget(watcher);
    }

    rx
}
