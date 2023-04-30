pub mod database;

pub type MagicError = Box<dyn std::error::Error>;

// TODO its getting really iritating to NOT be able to use impl in the place i want/need to
//pub type SpookyError = Box<impl std::error::Error>;
pub mod err_tools {
    use anyhow::anyhow;
    use axum::{http::StatusCode, response::IntoResponse};

    // Custom Errors
    #[derive(Debug)]
    pub struct ErrorH {
        pub original_error: anyhow::Error,
        // TODO Change this(ErrorH.code) to the http type or a usize maybe?
        pub descriptor: String,
        pub code: StatusCode,
    }
    impl ErrorH {
        pub fn new(original_error: anyhow::Error, code: StatusCode) -> ErrorH {
            let descriptor = original_error.to_string();
            ErrorH {
                original_error,
                descriptor,
                code,
            }
        }
        pub fn new_err(descriptor: &str, code: StatusCode) -> ErrorH {
            let original_error = anyhow!("{}", code.to_string());
            let descriptor = descriptor.to_string();
            Self {
                original_error,
                descriptor,
                code,
            }
        }
        pub fn new_box(original_error: anyhow::Error, code: StatusCode) -> Box<ErrorH> {
            let descriptor = original_error.to_string();
            Box::new(ErrorH {
                original_error,
                descriptor,
                code,
            })
        }
    }
    impl std::fmt::Display for ErrorH {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "{}", self.original_error.to_string())
        }
    }

    impl std::error::Error for ErrorH {
        fn description(&self) -> &str {
            self.descriptor.as_str()
        }
    }
    //pub trait HandleError<T> {
    //    fn to_errorh(self, code: StatusCode) -> Result<T, ErrorH>;
    //}

    // TODO When rust releases "Return 'Impl Trait' make this return impl trait Error"?
    //impl<T> HandleError<T> for Result<T, sqlx::error::Error> {
    //    fn to_errorh(self, code: StatusCode) -> Result<T, ErrorH> {
    //        match self {
    //            Ok(t) => Ok(t),
    //            Err(err) => Err(ErrorH::new(anyhow::Error::new(err), code)),
    //        }
    //    }
    //}
    //impl<T> HandleError<T> for Result<T, serde_json::Error> {
    //    fn to_errorh(self, code: StatusCode) -> Result<T, ErrorH> {
    //        match self {
    //            Ok(t) => Ok(t),
    //            Err(err) => Err(ErrorH::new(anyhow::Error::new(err), code)),
    //        }
    //    }
    //}

    // Handle X (Its Experimental)
    pub trait HandleError<T, E>
    where
        E: std::error::Error + std::marker::Sync + std::marker::Send + 'static,
    {
        fn to_errorh(self, code: StatusCode) -> Result<T, ErrorH>;
    }
    impl<T, E> HandleError<T, E> for Result<T, E>
    where
        E: std::error::Error + std::marker::Sync + std::marker::Send + 'static,
    {
        fn to_errorh(self, code: StatusCode) -> Result<T, ErrorH> {
            match self {
                Ok(t) => {
                    println!("to_errorh was OK");
                    return Ok(t);
                }
                Err(err) => {
                    println!("to_errorh was ERR: {}", err.to_string());
                    return Err(ErrorH::new(anyhow::Error::new(err), code));
                }
            }
        }
    }

    impl IntoResponse for ErrorH {
        fn into_response(self) -> axum::response::Response {
            (self.code, self.descriptor).into_response()
        }
    }

    pub fn err_on_false(ok: bool, msg: &str, code: StatusCode) -> Result<(), ErrorH> {
        if !ok {
            return Err(ErrorH::new_err(msg, code));
        }
        Ok(())
    }
}

pub mod time_tools {
    use crate::err_tools::{ErrorH, HandleError};
    use axum::http::StatusCode;
    use chrono::prelude::*;

    fn time_standard() -> String {
        // DATE: Year, Month, Day
        // TIME: Hour, Minute, Seconds
        let standard = "%Y-%m-%d %H:%M:%S";

        // Convert
        standard.to_string()
    }

    /// Get the time right NOW!
    pub fn time_of_day() -> String {
        // Set the time format standard
        let standard = &time_standard();

        Utc::now().naive_local().format(standard).to_string()
    }

    /// Add an offset to change the time of day.
    ///
    /// Currently it only does seconds by increase.
    pub fn time_of_day_n(seconds: i64) -> Result<String, ErrorH> {
        let standard = &time_standard();

        // An offset to increase/decrease the current (TIME OF DAY)
        let offset = chrono::Duration::seconds(seconds);

        // On Error, you caused an integer overflow. How&WHY?? lol
        let code = StatusCode::INTERNAL_SERVER_ERROR;

        let time = Utc::now()
            .naive_local()
            .checked_add_signed(offset)
            .ok_or(ErrorH::new_err(
                "You Int OverFlowed in time_of_dayx...",
                code,
            ))?
            .format(&standard)
            .to_string();

        Ok(time)
    }

    pub fn string_to_naive_date_time(time: &str) -> Result<NaiveDateTime, ErrorH> {
        // Set the standard to format the time by
        let standard = &time_standard();

        // Convert time to a Native Date Time (ndt)
        let ndt = Utc
            .datetime_from_str(time, standard)
            .to_errorh(StatusCode::INTERNAL_SERVER_ERROR)?
            .naive_local();

        Ok(ndt)
    }

    pub enum Equality {
        Left,
        Right,
        Equal,
    }
    use Equality::*;

    pub fn compare_time(left: &str, right: &str) -> Result<Equality, ErrorH> {
        // Convience name
        let convert = string_to_naive_date_time;

        // Failing to convert is a server error?
        let code = StatusCode::INTERNAL_SERVER_ERROR;

        // TODO properly handle errors
        let (left, right) = (
            convert(left).to_errorh(code)?,
            convert(right).to_errorh(code)?,
        );

        // Return proper equality
        let equality = if left == right {
            Equal
        } else if left > right {
            Left
        } else {
            // right > left
            Right
        };

        Ok(equality)
    }
    pub fn subtract_time(subtract_me: &str, subtract_by: &str) -> Result<String, ErrorH> {
        let left_time = string_to_naive_date_time(subtract_me)?;
        let right_time = string_to_naive_date_time(subtract_by)?;
        let new_time = (left_time - right_time).to_string();
        Ok(new_time)
    }
    use serde_derive::Serialize;
    #[derive(Serialize, Default)]
    pub struct Time {
        second: String,
        minute: String,
        hour: String,
    }

    impl Time {
        pub fn from_time_string(time: &str) -> Result<Self, ErrorH> {
            let time = &string_to_naive_date_time(time)?;
            let local_time = Local.from_local_datetime(time).unwrap();
            Ok(Time {
                second: local_time.second().to_string(),
                minute: local_time.minute().to_string(),
                hour: local_time.minute().to_string(),
            })
        }
    }
}
