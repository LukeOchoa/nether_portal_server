// Herp Derp
use crate::err_tools::{ErrorH, HandleError};
use crate::time_tools::{time_of_day, time_of_day_n};

// Squeel-X
use sqlx::postgres::{PgPoolOptions, Postgres};
use sqlx::{FromRow, Pool, Row};
// use sqlx::{FromRow, Pool, Row};

// Uniques
use uuid::Uuid;

use axum::http::StatusCode;

pub async fn db_connection_async() -> Result<Pool<Postgres>, ErrorH> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://hyacinth:free144@localhost/breaker")
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn check_if_exists(
    table: &str,
    column: &str,
    value: &str,
    pool: &Pool<Postgres>,
) -> Result<bool, ErrorH> {
    // Sql that checks if the specified table exists
    let sql = &format!(
        "SELECT exists (SELECT 1 FROM {} WHERE {}='{}');",
        table, column, value
    );

    // Query database and convert error to ErrorH with response code
    let pg_row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get value from PgRow
    let booly: bool = pg_row.get(0);

    // Return bool, true==yes, false==no
    return Ok(booly);
}

pub async fn select_from_db(
    column: &str,
    table: &str,
    condition: &str,
    where_condition: &str,
    pool: &Pool<Postgres>,
) -> Result<String, ErrorH> {
    // Sql that gets a single value from the table
    let sql = &format!(
        "SELECT {} FROM {} WHERE {}='{}';",
        column, table, condition, where_condition
    );

    // Query database and convert error to ErrorH with response code
    let pg_row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::FORBIDDEN)?;

    // Get value from PgRow
    let db_value: String = pg_row.get(0);

    // Return a String result
    Ok(db_value)
}

pub async fn get_valid_id(db_name: &str, pool: &Pool<Postgres>) -> Result<(i32, String), ErrorH> {
    let sql = &format!("SELECT id FROM {};", db_name);

    let mut pg_rows = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(|pg_row| pg_row.get(0))
        .collect::<Vec<i32>>();

    // Algo
    // I only want usize(positive integer) values inside the database
    // And we want the next lowest value available
    // If the list is sorted, you can just increment the loop until
    // you no longer equal the current element then you break and thats your next lowest value
    // Slow but easy to understand and change if needed

    // The lowest value in the database for ids; ids can never be below 0
    let mut lowest_new_value = 0;

    // Sort the list
    pg_rows.sort();

    for id in pg_rows.iter() {
        if lowest_new_value != *id {
            break;
        }
        lowest_new_value = lowest_new_value + 1
    }

    Ok((lowest_new_value, lowest_new_value.to_string()))
}

pub fn session_length() -> i64 {
    // Seconds
    let session_length_x = 90;

    session_length_x
}

pub async fn create_session_key(
    username: String,
    id: String,
    pool: &Pool<Postgres>,
) -> Result<(String, String), ErrorH> {
    struct SessionKey {
        id: String,
        session_id: String,
        last_active: String,
        expiration: String,
        username: String,
    }

    // Initialize struct (Just for convenience)
    let sk = SessionKey {
        id,
        session_id: Uuid::new_v4().to_string(),
        last_active: time_of_day(),
        expiration: time_of_day_n(session_length())?,
        username,
    };

    // Format SQL
    let columns = "id, sessionid, lastactive, expiration, username";
    let values = format!(
        "'{}', '{}', '{}', '{}', '{}'",
        sk.id, sk.session_id, sk.last_active, sk.expiration, sk.username
    );
    let sql = &format!(
        "INSERT INTO native_user_keys ({}) VALUES({});",
        columns, values
    );

    // Execute Insert Statement
    sqlx::query(sql)
        .execute(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((sk.session_id, sk.expiration))
}

pub async fn delete_session(
    where_: &str,
    where_con: &str,
    pool: &Pool<Postgres>,
) -> Result<(), ErrorH> {
    // Format SQL
    let sql = &format!(
        "DELETE FROM native_user_keys WHERE {}='{}';",
        where_, where_con
    );

    // Execute SQL
    sqlx::query(sql)
        .execute(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

pub async fn select_count(
    table: &str,
    column: &str,
    group_by: &str,
    where_x: Option<(&str, &str)>,
    pool: &Pool<Postgres>,
) -> Result<i32, ErrorH> {
    let select_from = format!("SELECT '{}', count('{}') FROM {}", column, column, table);

    let group_by = format!(" GROUP BY {}", group_by);

    // A where/where_condition does not always need to be supplied? apparently lol, i dont remember
    let where_x = where_x
        .and_then(|(s1, s2)| Some(format!("WHERE {}='{}'", s1, s2)))
        .unwrap_or_default();

    // Compose SQL Statement
    let sql = &format!("{}{}{}", select_from, group_by, where_x);

    // I dont know how to write SQL that just takes the count and not the whole row. Until i take the time to not be lazy we have this junk row lol!
    #[derive(FromRow)]
    struct Count {
        _waste: String,
        count: i32,
    }
    // Query database
    let count: Count = sqlx::query_as(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(count.count)
}

pub async fn execute_sql(sql: &str, pool: &Pool<Postgres>) -> Result<(), ErrorH> {
    sqlx::query(sql)
        .execute(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}

use sqlx::Column;
use std::collections::HashMap;
pub async fn get_npt_as_hashmap(
    sql: &str,
    pool: &Pool<Postgres>,
) -> Result<HashMap<String, HashMap<String, String>>, ErrorH> {
    // Get all the things from the Database
    let rows = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Container for all-the-things
    let mut payload: HashMap<String, HashMap<String, String>> = HashMap::new();

    // For each column in the table
    rows.into_iter().enumerate().for_each(|(key, row)| {
        println!("rows: {}", row.len());
        // for each row in the column
        let mut sub: HashMap<String, String> = HashMap::new();
        (0..row.len()).into_iter().for_each(|index| {
            // Get the column value; Convert the Int Or Text into a (rust String)
            let cvalue: String = row
                .try_get(index)
                .unwrap_or_else(|_| row.get::<i32, usize>(index).to_string());

            // Get the column name
            let cname = row.column(index).name().to_string();

            // Append to inner hashmap
            sub.insert(cname, cvalue);
        });
        // Append if to the soon-to-be-Jsonified HashMap
        payload.insert(key.to_string(), sub);
    });

    // Return all the things
    Ok(payload)
}

pub async fn rows_in_a_table(table: &str, pool: &Pool<Postgres>) -> Result<String, ErrorH> {
    let sql = &format!("SELECT count(*) AS exact_count FROM public.{}", table);

    // INT8 == i64; The database count type is INT8 so i32 will not work for converting it with the .get(x) method
    let count: i64 = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?
        .get(0);

    Ok(count.to_string())
}
