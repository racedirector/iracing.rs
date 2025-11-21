use iracing::telemetry::IBT;

pub fn main() {
    let ibt = IBT::open("./telemetry.ibt").expect("Could not open IBT from path");
    let header = ibt.header().expect("Could not get IBT header");
    println!("{:#?}", header);
    let sub_header = ibt.sub_header().expect("Could not get IBT sub-header");
    println!("{:#?}", sub_header);

    // let session_info = ibt.session_info().expect("Could not retrieve session data");
    // print!("{:#?}", session_info);
}
