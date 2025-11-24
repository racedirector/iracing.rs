use iracing::telemetry::IBT;

pub fn main() {
    let ibt = IBT::open("./telemetry.ibt").expect("Could not open IBT from path");
    println!("{:#?}", ibt.header);
    println!("{:#?}", ibt.sub_header);
}
