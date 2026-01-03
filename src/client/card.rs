use super::base_client::{Client, ClientBehaviour};
use colored::{ColoredString, Colorize};
use std::{sync::Arc, thread, time::Duration, vec};
use wg_2024::packet::PacketType;

const CARD_WIDTH: usize = 50;

#[derive(Clone, Copy, Debug)]
pub enum Rarity {
    Common,
    Rare,
    Quacking,
}

impl Rarity {
    fn apply_color(&self, string: &str) -> ColoredString {
        match self {
            Rarity::Common => string.green(),
            Rarity::Rare => string.blue(),
            Rarity::Quacking => string.bright_yellow(),
        }
    }

    fn to_prob_value(self) -> u32 {
        match self {
            Rarity::Common => 3,
            Rarity::Rare => 2,
            Rarity::Quacking => 1,
        }
    }
}

pub trait ActivationFunction<B>: Send + Sync
where
    B: ClientBehaviour,
{
    fn activate(&self, base_client: &mut Client<B>);
}

impl<F, B> ActivationFunction<B> for F
where
    F: Fn(&mut Client<B>) + Send + Sync,
    B: ClientBehaviour,
{
    fn activate(&self, base_client: &mut Client<B>) {
        self(base_client);
    }
}

pub struct Card<B>
where
    B: ClientBehaviour + Sized,
{
    title: &'static str,
    description: &'static str,
    rarity: Rarity,
    activation: Arc<dyn ActivationFunction<B>>,
}

impl<B> Clone for Card<B>
where
    B: ClientBehaviour,
{
    fn clone(&self) -> Self {
        Card {
            title: self.title,
            description: self.description,
            rarity: self.rarity,
            activation: self.activation.clone(),
        }
    }
}

impl<B> Card<B>
where
    B: ClientBehaviour,
{
    pub fn new(
        title: &'static str,
        description: &'static str,
        rarity: Rarity,
        activation: impl ActivationFunction<B> + 'static,
    ) -> Self {
        Card {
            title,
            description,
            rarity,
            activation: Arc::new(activation),
        }
    }

    pub fn is_yield_turn(&self) -> bool {
        self.title == "Yield Turn"
    }

    pub fn is_forget_topology(&self) -> bool {
        self.title == "Forget Topology"
    }

    pub fn prob_value(&self) -> u32 {
        self.rarity.to_prob_value()
    }

    pub fn activate(&self, base_client: &mut Client<B>) {
        self.activation.activate(base_client);
    }

    fn print_header(&self, index: usize) {
        let mut title = self.rarity.apply_color(self.title).bold();

        let fg_color = title.fgcolor.take();
        let bg_color = title.bgcolor.take();
        let style = std::mem::take(&mut title.style);

        let mut header =
            format!(" #{index:<2}{title:^width$}    ", width = CARD_WIDTH - 12,).normal();

        header.fgcolor = fg_color;
        header.bgcolor = bg_color;
        header.style = style;

        print!("{}", self.rarity.apply_color("│").bold());
        print!(" {} ", header);
        println!("{}", self.rarity.apply_color("│").bold());
    }
    fn print_description(&self) {
        let mut lines = Vec::<String>::new();
        for word in self.description.to_string().split_whitespace() {
            match lines.last_mut() {
                Some(last_line) if last_line.len() + word.len() < CARD_WIDTH - 4 => {
                    last_line.push_str(&format!(" {}", word));
                }
                _ => {
                    lines.push(word.to_string());
                }
            }
        }
        for line in lines.into_iter() {
            let colored_line = line.italic();

            print!("{}", self.rarity.apply_color("│").bold());
            print!(" {:^width$} ", colored_line, width = CARD_WIDTH - 4);
            println!("{}", self.rarity.apply_color("│").bold());
        }
    }
    pub fn print_card(&self, index: usize) {
        // ╭──────────╮
        print!("{}", self.rarity.apply_color("╭").bold());
        print!(
            "{}",
            self.rarity.apply_color(&"─".repeat(CARD_WIDTH - 2)).bold()
        );
        println!("{}", self.rarity.apply_color("╮").bold());

        // │   title  │
        self.print_header(index);

        // ├──────────┤
        print!("{}", self.rarity.apply_color("├").bold());
        print!(
            "{}",
            self.rarity.apply_color(&"─".repeat(CARD_WIDTH - 2)).bold()
        );
        println!("{}", self.rarity.apply_color("┤").bold());

        // │          │
        print!("{}", self.rarity.apply_color("│").bold());
        print!("{}", " ".repeat(CARD_WIDTH - 2));
        println!("{}", self.rarity.apply_color("│").bold());

        // │   desc   │
        self.print_description();

        // │          │
        print!("{}", self.rarity.apply_color("│").bold());
        print!("{}", " ".repeat(CARD_WIDTH - 2));
        println!("{}", self.rarity.apply_color("│").bold());

        // ╰──────────╯
        print!("{}", self.rarity.apply_color("╰").bold());
        print!(
            "{}",
            self.rarity.apply_color(&"─".repeat(CARD_WIDTH - 2)).bold()
        );
        println!("{}", self.rarity.apply_color("╯").bold());
    }
}
pub fn yield_turn_card<B>() -> Card<B>
where
    B: ClientBehaviour,
{
    Card::new(
        "Yield Turn",
        "Yield your turn to the next player",
        Rarity::Quacking,
        |_: &mut Client<B>| {},
    )
}

pub fn flood_request_card<B>() -> Card<B>
where
    B: ClientBehaviour,
{
    Card::new(
        "The Explorer",
        "Send a FloodRequest out",
        Rarity::Quacking,
        |base_client: &mut Client<B>| {
            base_client.initiate_flood();

            thread::sleep(Duration::from_millis(1500));

            let mut count = 0;

            while let Ok(packet) = base_client.try_recv_packet() {
                if let PacketType::FloodResponse(_) = &packet.pack_type {
                    count += 1;
                }
                base_client.handle_packet_normal(packet);
            }

            println!("{count} FloodResponses received");
        },
    )
}

pub fn the_navigator_card<B>() -> Card<B>
where
    B: ClientBehaviour,
{
    Card::new(
        "The Navigator",
        "Calculate the routes towards all reachable servers",
        Rarity::Rare,
        |base_client: &mut Client<B>| {
            let routes_count = base_client.calculate_routes();

            println!("{} routes found!", routes_count);

            thread::sleep(Duration::from_millis(500));
        },
    )
}

pub fn generic_cards<B>() -> Vec<Card<B>>
where
    B: ClientBehaviour,
{
    vec![
        flood_request_card(),
        the_navigator_card(),
        Card::new(
            "Forget Topology",
            "Forget the current topology and draw a The Explorer card",
            Rarity::Rare,
            |base_client: &mut Client<B>| {
                base_client.forget_topology();

                println!("You forgot the network topology!");

                println!("You draw a The Explorer card!");

                thread::sleep(Duration::from_millis(500));
            },
        ),
        Card::new(
            "Servers",
            "Discover the servers in the network",
            Rarity::Common,
            |base_client: &mut Client<B>| {
                base_client.print_reachable_servers();

                thread::sleep(Duration::from_millis(500));
            },
        ),
    ]
}
