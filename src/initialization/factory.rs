#[macro_export]
macro_rules! drone_factories {
    ($($type_name:ty),* $(,)?) => {{
        vec![
            $(
                Box::new(
                    |id, csend, crecv, precv, psend, pdr| -> Box<dyn Runnable> {
                        Box::new(<$type_name>::new(id, csend, crecv, precv, psend, pdr))
                    }
                ) as Box<dyn DroneCreatorFunction>
            ),*
        ]
    }};
}

#[macro_export]
macro_rules! client_factories {
    ($($type_name:ty),* $(,)?) => {{
        vec![
            $(
                Box::new(
                    |id, csend, crecv, precv, psend, turn_handler| -> Box<dyn Runnable> {
                        Box::new(<$type_name>::with_default_behaviour(id, csend, crecv, precv, psend, turn_handler))
                    }
                ) as Box<dyn ClientCreatorFunction>
            ),*
        ]
    }};
}

#[macro_export]
macro_rules! server_factories {
    ($($type_name:ty),* $(,)?) => {{
        vec![
            $(
                Box::new(
                    |id, csend, crecv, precv, psend| -> Box<dyn Runnable> {
                        Box::new(<$type_name>::with_default_behaviour(id, csend, crecv, precv, psend))
                    }
                ) as Box<dyn ServerCreatorFunction>
            ),*
        ]
    }};
}
