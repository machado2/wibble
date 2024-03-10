use crate::create::create_routes;
use crate::newslist::news_list;
use crate::newslist::news_list_pt;
use crate::content::get_content;

pub fn get_routes() -> Vec<rocket::Route> {
    let mut r = create_routes();
    r.append(&mut routes![news_list, get_content, news_list_pt]);
    r
}
