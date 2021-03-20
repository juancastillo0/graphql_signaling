use graphql_test::{books, spawn_test_app};
use reqwest::{header::CONTENT_TYPE, Client};
use serde_json::from_str;

#[actix_rt::test]
async fn get_token() {
    let app = spawn_test_app().await.expect("Failed to start app");
    let client = Client::new();
    let test_token = "dwdaiubisajnd-dawonaw";

    let response = client
        .post(&app.base_url())
        .header("token", test_token)
        .header(CONTENT_TYPE, "application/text")
        .body(r#" { "query": " { token } " } "#)
        .send()
        .await
        .expect("Failed to send request");
    let status = response.status();
    let body_str = response.text().await.unwrap();
    assert_eq!(status.as_u16(), 200_u16);

    (|| -> Option<()> {
        let body: serde_json::Value = from_str(&body_str).ok()?;
        assert_eq!(
            body.as_object()?
                .get("data")?
                .as_object()?
                .get("token")?
                .as_str(),
            Some(test_token)
        );
        Some(())
    })()
    .unwrap()
}

async fn get_books(base_url: &str, client: &Client) -> Vec<books::Book> {
    let response = client
        .post(base_url)
        .body(r#" { "query": " { books { id, name, author } } " } "#)
        .send()
        .await
        .expect("Failed to send request");
    let status = response.status();
    let body_str = response.text().await.unwrap();
    assert_eq!(status.as_u16(), 200_u16);

    let body_value = (|| -> Option<serde_json::Value> {
        let body: serde_json::Value = from_str(&body_str).ok()?;
        Some(
            body.as_object()?
                .get("data")?
                .as_object()?
                .get("books")?
                .clone(),
        )
    })()
    .unwrap();

    serde_json::from_value::<Vec<books::Book>>(body_value).unwrap()
}

#[actix_rt::test]
async fn create_books() {
    let app = spawn_test_app().await.expect("Failed to start app");
    let client = Client::new();

    let mut books = get_books(&app.base_url(), &client).await;
    assert_eq!(books.len(), 0);

    // dbg!(body_req);
    let response = client
        .post(&app.base_url())
        .header(CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({"query": r#"mutation {createBook(name: "nn", author: "aa b")}"#})
                .to_string(),
        )
        .send()
        .await
        .expect("Failed to send request");
    let status = response.status();
    let body_str = response.text().await.unwrap();

    assert_eq!(status.as_u16(), 200_u16);
    let body: serde_json::Value = from_str(&body_str).unwrap();
    let book_id = (|| -> Option<&str> {
        body.as_object()?
            .get("data")?
            .as_object()?
            .get("createBook")?
            .as_str()
    })()
    .expect(&format!("{:?}", body));

    books = get_books(&app.base_url(), &client).await;
    assert_eq!(books.len(), 1);
    let book = &books[0];
    assert_eq!(book.name, "nn");
    assert_eq!(book.author, "aa b");
    assert_eq!(book.id, book_id);
}
