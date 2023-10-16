pub trait DropHandler<T> {
    fn drop_handler(obj: &mut T);
}

pub struct DefaultDropHandler;

impl<T> DropHandler<T> for DefaultDropHandler {
    fn drop_handler(_obj: &mut T) {}
}
