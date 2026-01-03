use rand::Rng;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Scraper {
    pub data: HashMap<String, Vec<u8>>,
}
impl Scraper {
    pub fn new() -> Scraper {
        Scraper {
            data: HashMap::new(),
        }
    }
    pub fn get_image(&mut self, url: &str) -> Option<Vec<u8>> {
        let resp = attohttpc::get(url).send();
        let bytes = resp.unwrap().bytes().unwrap();
        // let path = Path::new(path);
        // Scraper::write_to_file(
        //     path.display().to_string(),
        //     bytes.clone()).expect("write error");
        Some(bytes)
    }
    pub fn get_html(&mut self, url: &str) -> Option<String> {
        let resp = attohttpc::get(url).send();
        let bytes = resp.unwrap().text().unwrap();
        Self::get_urls(self, bytes.clone());
        Some(bytes)
    }

    pub fn get_urls(&mut self, html: String) -> Vec<(String, String)> {
        let document = scraper::Html::parse_document(&html);

        let html_product_selector = scraper::Selector::parse("li.product").unwrap();
        let html_products = document.select(&html_product_selector);
        let mut urls = Vec::<(String, String)>::new();
        for product in html_products {
            let product_name = product
                .select(&scraper::Selector::parse("h2").unwrap())
                .next()
                .map(|a| a.text().collect::<String>());
            let image_url = product
                .select(&scraper::Selector::parse("img").unwrap())
                .next()
                .and_then(|a| a.value().attr("src"))
                .map(str::to_owned);
            urls.push((product_name.unwrap(), image_url.unwrap()));
            // println!("{:?}", image_url);
        }
        urls
    }
    pub fn insert_urls(&mut self, urls: Vec<(String, String)>) -> Vec<(String, String)> {
        let mut res = Vec::<(String, String)>::new();
        let mut rng = rand::thread_rng();
        let mut taken: Vec<usize> = Vec::new();
        while taken.len() != 4 {
            let n = rng.gen_range(0..urls.len());
            if !taken.contains(&n) {
                res.push((urls[n].0.clone(), urls[n].1.clone()));
                taken.push(n);
            }
        }
        for (name, address) in res.clone().iter() {
            let bytes = self.get_image(address);
            self.data.entry(name.clone()).or_insert(bytes.unwrap());
        }
        res
    }
}

#[cfg(test)]
mod scraper_test {
    use crate::message::base_message::Message;
    use crate::message::content_message::ContentRequest;
    use crate::message::media_message::MediaRequest;
    use crate::server::base_server::ServerBehaviour;
    use crate::server::media_server::MediaServerBehaviour;

    #[test]
    fn scrape_test() {
        let mut server = MediaServerBehaviour::default();
        let message = server.handle_request(
            Message::new(
                0,
                0,
                0,
                ContentRequest::MediaRequest(MediaRequest::ExpandList),
            ),
            0,
        );
        println!("{:?}", message);
    }
}
