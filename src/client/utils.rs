use std::{
    fmt::{Debug, Display},
    io::stdin,
    str::FromStr,
};

fn read_line() -> String {
    let mut string = String::new();
    stdin().read_line(&mut string).ok();
    string
}

pub fn input_with_check<T, F>(prompt: String, check: F) -> T
where
    T: FromStr<Err: Debug> + Display,
    F: Fn(&T) -> bool,
{
    println!("{prompt}");

    let res = T::from_str(read_line().trim());

    match res {
        Ok(index) if check(&index) => index,
        _ => {
            println!("Invalid input, try again");
            input_with_check(prompt, check)
        }
    }
}

pub fn input<T>(prompt: String) -> T
where
    T: FromStr<Err: Debug> + Display,
{
    println!("{}", prompt);

    let res = T::from_str(read_line().trim());

    match res {
        Ok(index) => index,
        _ => {
            println!("Invalid input, try again");
            input(prompt)
        }
    }
}

pub fn wait_for_input() {
    println!("Press enter to continue...");
    read_line();
}
