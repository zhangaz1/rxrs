use std::marker::PhantomData;
use std::sync::Arc;
use crate::*;
use crate::util::any_send_sync::AnySendSync;
use std::ops::Deref;

pub struct MapOp<SS, VBy, Src, F>
{
    f: Arc<F>,
    src: Src,
    PhantomData: PhantomData<(SS, AnySendSync<VBy>)>
}

pub struct DynObservable<'s, 'o, SS:YesNo, By: RefOrVal, EBy: RefOrVal>
{
    src: Box<Observable<'o, SS, By, EBy> + 's>,
}

impl<'s, 'o, SS:YesNo, By: RefOrVal, EBy: RefOrVal> DynObservable<'s, 'o, SS, By, EBy>
{
    pub fn new(src: impl Observable<'o, SS, By, EBy>+'s) -> Self { DynObservable{ src: Box::new(src) }}
    pub fn from_box(src: Box<Observable<'o, SS, By, EBy>+'s>) -> Self { DynObservable{ src }}
}


impl<'s, 'o, SS:YesNo, By: RefOrVal, EBy: RefOrVal> Deref for DynObservable<'s, 'o, SS, By, EBy>
{
    type Target = Observable<'o, SS, By, EBy> + 's;

    fn deref(&self) -> &Self::Target {
        self.src.as_ref()
    }
}

//impl<'s, 'o, SS:YesNo, By: RefOrVal, EBy: RefOrVal> Observable<'o, SS, By, EBy> for X<'s, 'o, SS, By, EBy>
//{
//    fn sub(&self, next: impl ActNext<'o, SS, By>, err_or_comp: impl ActEc<'o, SS, EBy>) -> Unsub<'o, SS> where Self: Sized {
//        self.src.sub_dyn(box next, box err_or_comp)
//    }
//
//    fn sub_dyn(&self, next: Box<ActNext<'o, SS, By>>, err_or_comp: Box<ActEcBox<'o, SS, EBy>>) -> Unsub<'o, SS> {
//        self.src.sub_dyn(next, err_or_comp)
//    }
//}




pub trait ObsMapOp<'o, SS: YesNo, VBy: RefOrVal, EBy: RefOrVal, VOut, F: Act<SS, VBy, VOut>+'o> : Sized
{
    fn map(self, f: F) -> MapOp<SS, VBy, Self, F> { MapOp{ f: Arc::new(f), src: self, PhantomData } }
}
pub trait DynObsMapOp<'o, SS: YesNo, VBy: RefOrVal+'o, EBy: RefOrVal+'o, VOut:'o, F: Act<SS, VBy, VOut>+'o>
{
    fn map(self, f: F) -> DynObservable<'o, 'o, SS, Val<VOut>, EBy>;
}

impl<'o, SS:YesNo, VBy: RefOrVal+'o, EBy: RefOrVal+'o, VOut, Src: Observable<'o, SS, VBy, EBy>+'o, F: Act<SS, VBy, VOut>+'o>
ObsMapOp<'o, SS, VBy,EBy, VOut, F>
for Src
{}

impl<'o, SS:YesNo, VBy: RefOrVal+'o, EBy: RefOrVal+'o, VOut:'o, F: Act<SS, VBy, VOut>+'o>
DynObsMapOp<'o, SS, VBy,EBy, VOut, F>
for DynObservable<'o, 'o, SS, VBy, EBy>
{
    fn map(self, f: F) -> DynObservable<'o, 'o, SS, Val<VOut>, EBy>
    {
        DynObservable::new(MapOp{ f: Arc::new(f), src: self.src, PhantomData })
    }
}




impl<'s, 'o, SS:YesNo, VOut: 'o, VBy: RefOrVal+'o, EBy: RefOrVal+'o, Src: Observable<'o, SS, VBy, EBy>+'s, F: Act<SS, VBy, VOut>+'o>
Observable<'o, SS, Val<VOut>, EBy>
for MapOp<SS, VBy, Src, F>
{
    fn sub(&self, next: impl ActNext<'o, SS, Val<VOut>>, ec: impl ActEc<'o, SS, EBy>) -> Unsub<'o, SS> where Self: Sized
    {
        let next = SSActNextWrap::new(next);
        let f = act_sendsync(self.f.clone());
        let sub = Unsub::new();

        sub.clone().added_each(self.src.sub(forward_next(next, (sub, f), |next, (sub, f), v: VBy| {
            let v = f.call(v.into_v());
            sub.if_not_done(|| next.call(v));
        }, |s,_|s.stopped()), ec))
    }

    fn sub_dyn(&self, next: Box<ActNext<'o, SS, Val<VOut>>>, ec: Box<ActEcBox<'o, SS, EBy>>) -> Unsub<'o, SS>
    { self.sub(next, ec) }
}

#[cfg(test)]
mod test
{
    use crate::*;
    use std::cell::RefCell;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::*;

    #[test]
    fn smoke()
    {
        let n = Cell::new(0);
        let o = Of::value(123);
        o.map(|v: &_| *v * 2).sub(|v| { n.replace(v);}, ());
        assert_eq!(n.get(), 246);

        let o = Of::value("B".to_owned());

        let result = RefCell::new(String::new());
        let mapped = o.into_dyn().map(|s:&_| format!("A{}", *s)).map(|s| format!("{}C", s));

        mapped.sub_dyn(box |v:String| result.borrow_mut().push_str(&v), box());

        assert_eq!(result.borrow().as_str(), "ABC");
    }

    #[test]
    fn unsub()
    {
        let n = Cell::new(0);
        let (i,o) = Rc::new(Subject::<NO, i32>::new()).clones();
        let unsub = o.map(|v:&_| *v+1).sub(|v| { n.replace(v); }, ());

        i.next(1);
        assert_eq!(n.get(), 2);

        unsub();
        i.next(2);
        assert_eq!(n.get(), 2);
    }

    #[test]
    fn boxed()
    {
        let o: Box<Observable<NO, Ref<i32>>> = Box::new(Of::value(123));

        let o = DynObservable { src: o};
        let o = o.map(|v:&_| v+1).map(|v| v*v);
        o.sub_dyn(box |v| println!("v={}", v), box ());
    }

    #[test]
    fn thread()
    {
        let (n, n1) = Arc::new(AtomicI32::new(0)).clones();
        let (i, o, send) = Arc::new(Subject::<YES, i32>::new()).clones();

        o.sub(|_: &_|{}, ());

        o.map(|v:&_| v+1).sub(move |v:i32| {n.store(v, Ordering::SeqCst); }, ());

        let s = send.map(|v:&_| v * v);
        ::std::thread::spawn(move ||{
            i.next(123);

            s.sub(|_|{}, ());
        }).join().ok();

        assert_eq!(n1.load(Ordering::SeqCst), 124);
    }

    #[test]
    fn drops_closure()
    {
        let (r, r1) = Rc::new(0).clones();

        assert_eq!(Rc::strong_count(&r), 2);

        let o = Of::value(123);

        o.map(move |_:&i32| Rc::strong_count(&r1)).sub((), ());

        assert_eq!(Rc::strong_count(&r), 1);
    }

    #[test]
    fn should_complete()
    {
        let (n1, n2, n3) = Rc::new(Cell::new(0)).clones();
        let (input, output) = Rc::new(Subject::<NO, i32>::new()).clones();

        output.map(move |v:&_| *v).sub(
            move |v| {  n1.replace(n1.get() + v); },
            move |_:Option<&_>| {  n2.replace(n2.get() + 1);  });

        input.next(1);
        input.next(2);

        assert_eq!(n3.get(), 3);

        input.complete();

        assert_eq!(n3.get(), 4);
    }
}
