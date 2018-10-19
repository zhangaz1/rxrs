use std::marker::PhantomData;
use std::sync::Arc;
use crate::*;
use std::sync::atomic::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct FilterOp<SS, Src, F>
{
    f: Arc<F>,
    src: Src,
    PhantomData: PhantomData<(SS)>
}

pub trait ObservableFilterOp<SS, VBy, EBy, F: Fn(&By<VBy>)->bool> : Sized
{
    fn filter(self, f: F) -> FilterOp<SS, Self, F> { FilterOp{ f: Arc::new(f), src: self, PhantomData} }
}

impl<'o, VBy: RefOrVal, EBy: RefOrVal, Src, F, SS:YesNo> ObservableFilterOp<SS, VBy,EBy, F> for Src
    where F: Fn(&By<VBy>)->bool+'o,
          Src: Observable<'o, SS, VBy, EBy>
{}


impl<'o, VBy: RefOrVal+'o, EBy:RefOrVal+'o, Src, F> Observable<'o, NO, VBy, EBy> for FilterOp<NO, Src, F>
    where F: Fn(&By<VBy>)->bool+'o,
          Src: Observable<'o, NO, VBy, EBy>
{
    fn sub(&self, next: impl ActNext<'o, NO, VBy>, ec: impl ActEc<'o, NO, EBy>) -> Unsub<'o, NO> where Self: Sized
    {
        let f = self.f.clone();
        let (done_in, done_out) = Rc::new(Cell::new(false)).clones();

        self.src.sub(
            move |v:By<_>         | if !done_in.get() && f(&v) { next.call(v); },
            move |e: Option<By<_>>| if !done_out.replace(true) { ec.call_once(e) }
        )
    }

    fn sub_dyn(&self, next: Box<ActNext<'o, NO, VBy>>, ec: Box<ActEcBox<'o, NO, EBy>>) -> Unsub<'o, NO>
    {
        self.sub(move |v:By<_>| next.call(v), move |e: Option<By<_>>| ec.call_box(e))
    }
}

impl<VBy: RefOrValSSs, EBy: RefOrValSSs, Src, F> Observable<'static, YES, VBy, EBy> for FilterOp<YES, Src, F>
    where F: Fn(&By<VBy>)->bool+'static+Send+Sync,
          Src: Observable<'static, YES, VBy, EBy>
{
    fn sub(&self, next: impl ActNext<'static, YES, VBy>, ec: impl ActEc<'static, YES, EBy>) -> Unsub<'static, YES> where Self: Sized
    {
        let (f, next, ec) = (self.f.clone(), ActSendSync::wrap_next(next), ActSendSync::wrap_ec(ec));
        let (done_in, done_out) = Arc::new(AtomicBool::new(false)).clones();

        self.src.sub(
            move |v:By<_>         | if !done_in.load(Ordering::Relaxed) && f(&v){ next.call(v); },
            move |e: Option<By<_>>| if !done_out.swap(true, Ordering::Release)  { ec.into_inner().call_once(e) }
        )
    }

    fn sub_dyn(&self, next: Box<ActNext<'static, YES, VBy>>, ec: Box<ActEcBox<'static, YES, EBy>>) -> Unsub<'static, YES>
    {
        let (next, ec) = (next.into_ss(), ec.into_ss());
        self.sub(move |v:By<_>| next.call(v), move |e: Option<By<_>>| ec.call_box(e))
    }
}


#[cfg(test)]
mod test
{
    use crate::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn smoke()
    {
        let n = Cell::new(0);
        let (input, output) = Rc::new(Subject::<NO, i32>::new()).clones();

        output.filter(|v| v.as_ref() % 2 == 0).sub(|v:By<_>| { n.replace(n.get() + *v); }, ());

        for i in 0..10 {
            input.next(i);
        }

        assert_eq!(n.get(), 20);
    }

    #[test]
    fn callback_safety()
    {
        let n = Cell::new(0);
        let (input, output, side_effect) = Rc::new(Subject::<NO, i32>::new()).clones();

        output.filter(move |v| {
            side_effect.complete();
            v.as_ref() % 2 == 0
        }).sub(|v:By<_>| { n.replace(n.get() + *v); }, ());

        for i in 0..10 {
            input.next(i);
        }

        assert_eq!(n.get(), 0);
    }
}
