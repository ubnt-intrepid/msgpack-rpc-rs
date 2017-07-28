use std::io;

error_chain!{
    foreign_links {
        StdIo(io::Error);
    }
}
