// Standard Library
use std::net::SocketAddr;

// Axum
use axum::http::StatusCode;
use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

// Squeel-X
// use sqlx::FromRow;

// Serde
use serde::{Deserialize, Serialize};
use serde_json::json;

// Bcrypt
extern crate bcrypt;
use bcrypt::verify;
// DEFAULT_COST, hash

// Herp Derp
use nether_portals_server::database::postgres::*;
use nether_portals_server::err_tools::{err_on_false, ErrorH, HandleError};
use nether_portals_server::time_tools::*;

// Big Boi Function
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(breaker))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/addnetherportaltext", post(add_nether_portal_text))
        .route("/getnetherportalstextinformation", get(get_npt_information))
        .route("/getnetherportalimagenames", get(get_npi_names))
        .route("/getaccessrights", get(get_access_rights))
        .route("/getsessiontimeleft", post(get_session_time_left))
        .route("/netherportalsestimatedamount", get(nps_estimated_amount))
        .route("/deleteimage", post(delete_image));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn breaker_x() -> serde_json::Value {
    json!({"Breaker": "Death to the breaker!"})
}
async fn breaker() -> Json<serde_json::Value> {
    println!("Breaker ping");
    Json(breaker_x())
}

#[derive(Deserialize)]
struct Profile {
    username: String,
    password: String,
    sessionkey: String,
}

async fn login(Json(payload): Json<serde_json::Value>) -> Result<Json<serde_json::Value>, ErrorH> {
    // Deserialize response to struct Profile
    let profile: Profile = serde_json::from_value(payload).unwrap();

    // Open a DB connection
    let pool = &db_connection_async().await?;

    // Check if the given username exists;
    let ok = check_if_exists("userprofile", "username", &profile.username, pool).await?;
    err_on_false(ok, "Username is bad", StatusCode::FORBIDDEN)?;

    // Get password in db based on their username
    let stored_password = select_from_db(
        "password",
        "userprofile",
        "username",
        &profile.username,
        pool,
    )
    .await?;

    // Check if the given password is correct
    verify(profile.password, &stored_password).to_errorh(StatusCode::FORBIDDEN)?;

    // Check if the user has a session
    let ok = check_if_exists("native_user_keys", "username", &profile.username, pool).await?;

    // Get the session key
    if ok {
        // Delete the session the person already has (Alternatively we could just perform an update)
        delete_session("username", &profile.username, pool).await?;
    }

    // Get a usable id
    let id = get_valid_id("native_user_keys", pool).await?.1;

    // Create a session
    let (key, time) = create_session_key(profile.username, id, pool).await?;

    // Struct for serialize-ing
    #[derive(Serialize)]
    struct Message {
        key: String,
        time: String,
    }

    // Create a json message to send the time, and session key to the client; it should never panic? maybe use .expect instead...
    let message =
        serde_json::to_value(Message { key, time }).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(message))
}

async fn logout(Json(payload): Json<serde_json::Value>) -> Result<impl IntoResponse, ErrorH> {
    // Deserialize response to struct Profile
    let profile: Profile = serde_json::from_value(payload).to_errorh(StatusCode::BAD_REQUEST)?;

    // Open a DB connection
    let pool = &db_connection_async().await?;

    // Check if user has a session (to actually delete)
    let ok = check_if_exists("native_user_keys", "sessionid", &profile.sessionkey, pool).await?;

    // If yes, then delete the session
    if ok {
        delete_session("username", &profile.username, pool).await?;
    }

    // Let the client know logout was successfull; change to status 202
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
struct Portal {
    xcord: i32,
    ycord: i32,
    zcord: i32,
    local: String,
    owner: String,
    notes: String,
    true_name: String,
}

impl Portal {
    fn to_array(&self) -> [String; 6] {
        let s = self;
        [
            s.xcord.to_string(),
            s.ycord.to_string(),
            s.zcord.to_string(),
            s.local.to_owned(),
            s.owner.to_owned(),
            s.notes.to_owned(),
        ]
    }
}

#[derive(Deserialize)]
struct NetherPortal {
    id: i32,
    nether: Portal,
    overworld: Portal,
    username: String,
}

impl NetherPortal {
    fn to_formated_string(&self) -> String {
        let mut nether = String::default();
        self.nether
            .to_array()
            .into_iter()
            .for_each(|s| nether.push_str(&format!("{}, ", s)));

        let mut overworld = String::default();
        self.overworld
            .to_array()
            .into_iter()
            .for_each(|s| overworld.push_str(&format!("{}, ", s)));

        format!("{}, {} {} {}", self.id, overworld, nether, self.username)
    }
    fn column_names() -> Vec<String> {
        [
            "id",
            "xcord_overworld",
            "ycord_overworld",
            "zcord_overworld",
            "xcord_nether",
            "ycord_nether",
            "zcord_nether",
            "local_overworld",
            "owner_overworld",
            "notes_overworld",
            "overworld_true_name",
            "local_nether",
            "owner_nether",
            "notes_nether",
            "nether_true_name",
            "username",
        ]
        .into_iter()
        .map(|s| s.to_owned())
        .collect()
    }
    fn database_insert_string(self, where_: Option<()>) -> String {
        let mut columns = String::default();

        // If there is a were con, handle it
        let where_ = where_
            .and(Some(format!("where id={}", self.id)))
            .unwrap_or_default();

        NetherPortal::column_names()
            .iter_mut()
            .for_each(|s| columns.push_str(&format!("{}{}", s, ",")));
        columns.remove(columns.len() - 1);

        let values = self.to_formated_string();
        format!(
            "INSERT INTO netherportals({}) VALUES({}){};",
            columns, values, where_
        )
    }
}

// TODO test this route lol. Shouldn't I use get_valid_id before I insert this into the database?
async fn add_nether_portal_text(
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ErrorH> {
    let nether_portal: NetherPortal =
        serde_json::from_value(payload).to_errorh(StatusCode::BAD_REQUEST)?;

    // Open a DB connection
    let pool = &db_connection_async().await?;

    // Get the current amount of records the user has for NetherPortals; MORE THAN 10 is DIS-allowed (i need a limit because its not excatly free yet...)
    let count = select_count("netherportals", "username", "username", None, pool).await?;

    // Exit if they have to many
    err_on_false(
        count > 9,
        "Too many profiles already...",
        StatusCode::FORBIDDEN,
    )?;

    // Create a SQL insert statement from (struct NetherPortal) data
    let sql = &nether_portal.database_insert_string(None);

    // Execute SQL statement
    execute_sql(sql, pool).await?;

    // Return a success code
    Ok(StatusCode::ACCEPTED)
}

#[derive(FromRow, Deserialize, Serialize)]
struct ImageDetails {
    id: i32,
    name: String,
    true_name: String,
    username: String,
}

impl ImageDetails {
    fn to_formated_string(self) -> String {
        format!(
            "{}, {}, {}, {}",
            self.id, self.name, self.true_name, self.username
        )
    }
    pub fn database_insert_string(self) -> String {
        let columns = format!("{}{}{}{}", "id,", "name,", "true_name,", "username");
        let values = self.to_formated_string();
        format!(
            "INSERT INTO netherportal_images({}) VALUES({})",
            columns, values
        )
    }
}

// TODO test function, i have no idea if it works yet...
async fn add_nether_portal_image_details(
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ErrorH> {
    let image_details: ImageDetails =
        serde_json::from_value(payload).to_errorh(StatusCode::BAD_REQUEST)?;

    // Open a DB connection
    let pool = &db_connection_async().await?;

    let count = select_count(
        "netherportal_images",
        "true_name",
        "true_name",
        Some(("usernam", &image_details.name)),
        pool,
    )
    .await?;
    err_on_false(count > 9, "Too many profiles...", StatusCode::FORBIDDEN)?;

    // Create a SQL insert statement from the given struct
    let sql = &image_details.database_insert_string();

    // Execute SQL statement
    execute_sql(sql, pool).await?;

    // Return some success code
    Ok(StatusCode::ACCEPTED)
}

// TODO test to see if this works
async fn save_npt_changes(
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ErrorH> {
    let nether_portal: NetherPortal =
        serde_json::from_value(payload).to_errorh(StatusCode::FORBIDDEN)?;

    let pool = &db_connection_async().await?;

    let sql = &nether_portal.database_insert_string(Some(()));

    execute_sql(sql, pool).await?;

    Ok(StatusCode::ACCEPTED)
}

use axum::extract::Query;
use sqlx::FromRow;
#[derive(Deserialize)]
struct Pag {
    orderby: Option<String>,
    limit: Option<String>,
}

// Get NetherPortal Text Information
async fn get_npt_information(Query(pag): Query<Pag>) -> Result<impl IntoResponse, ErrorH> {
    println!("Proc");
    let (orderby, limit) = (pag.orderby.unwrap(), pag.limit.unwrap());

    let pool = &db_connection_async().await?;

    let sql = &format!(
        "SELECT * FROM netherportals WHERE id > {} ORDER BY id LIMIT {};",
        orderby, limit
    );

    let payload = get_npt_as_hashmap(sql, pool).await?;
    println!("{:?}", payload);

    Ok(Json(
        serde_json::to_value(payload).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?,
    ))
}

#[derive(Deserialize)]
struct Name {
    true_name: String,
}

async fn get_npi_names(Query(name): Query<Name>) -> Result<impl IntoResponse, ErrorH> {
    let sql = &format!(
        "SELECT * FROM netherportal_images WHERE true_name='{}';",
        name.true_name
    );

    let pool = &db_connection_async().await?;

    let image_name: ImageDetails = sqlx::query_as(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    let payload = serde_json::to_value(image_name).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(payload))
}

// Passed
async fn delete_image(Json(payload): Json<serde_json::Value>) -> Result<impl IntoResponse, ErrorH> {
    let image_details: ImageDetails =
        serde_json::from_value(payload).to_errorh(StatusCode::BAD_REQUEST)?;

    let pool = &db_connection_async().await?;

    let sql = &format!(
        "DELETE FROM netherportal_images WHERE name='{}';",
        image_details.name
    );

    execute_sql(sql, pool).await?;

    //Ok(String::default())
    Ok(StatusCode::ACCEPTED)
}

#[derive(Serialize, Deserialize)]
struct Username {
    username: String,
}

use sqlx::Row;
async fn get_access_rights(Query(username): Query<Username>) -> Result<impl IntoResponse, ErrorH> {
    // Get the access rights (type []TEXT in DB) from the database
    let sql = &format!(
        "SELECT netherportals FROM netherportal_access_rights WHERE username='{}';",
        username.username
    );

    // Open a DB connection
    let pool = &db_connection_async().await?;

    // Query the database & convert the value
    let access_rights: Vec<String> = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::BAD_REQUEST)?
        .get(0);

    // Serialize the access_rights to json
    let payload =
        serde_json::to_value(access_rights).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return all the things
    Ok(Json(payload))
}

#[derive(Serialize, Deserialize)]
struct Key {
    key: String,
}
#[derive(Serialize)]
struct Session {
    key: String,
    time: Time,
}

// Tested... (passed)
async fn get_session_time_left(
    Json(Key { key }): Json<Key>,
) -> Result<Json<serde_json::Value>, ErrorH> {
    // Send this as a response at the end
    let mut time = Time::default();

    // if the user doesnt have a session, return a blank response
    if key == String::default() {
        let session = Session { key, time };
        let payload = serde_json::to_value(session).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;
        // Early return
        return Ok(Json(payload));
    }

    // Open DB connection
    let pool = &db_connection_async().await?;

    let time_as_string =
        &select_from_db("expiration", "native_user_keys", "sessionid", &key, pool).await?;

    // Check if the session time stored in the database is expired
    let time_equality = compare_time(time_as_string, &time_of_day())?;

    // If equality-right is greater, then delete the session; (forces user to re-login)
    if let Equality::Right = time_equality {
        delete_session("sessionid", &key, pool).await?;
    } else {
        // Get the remaining sesion time by subtracting the urrent time
        let time_string = &subtract_time(time_as_string, &time_of_day())?;

        // Create a time object to send back
        time = Time::from_time_string(time_string)?
    };

    // Convert to JSON
    let session = Session { key, time };
    let payload = serde_json::to_value(session).to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Return a Session Struct AS JSON
    Ok(Json(payload))
}

async fn nps_estimated_amount() -> Result<impl IntoResponse, ErrorH> {
    // Open DB connection
    let pool = &db_connection_async().await?;

    // Get a relative count for how many rows in the db there are (definely need to remove this in the future lol, very bad and ineficent)
    let rows_count = rows_in_a_table("netherportals", pool).await?;

    // Conveinence struct
    #[derive(Serialize)]
    struct Count {
        count: String,
    }

    // Serialize to JSON
    let payload = serde_json::to_value(Count { count: rows_count })
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(payload))
}
