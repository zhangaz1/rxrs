use std::rc::Rc;
use std::any::Any;
use subscriber::*;
use observable::*;
use unsub_ref::UnsubRef;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::marker::PhantomData;


pub struct TakeUntilState
{
    notified: Arc<AtomicBool>
}

pub struct TakeUntilOp<VNoti, Src, Noti>
{
    source : Src,
    noti: Noti,
    PhantomData: PhantomData<VNoti>
}

pub trait ObservableTakeUntil<V, Src, VNoti, Noti> where
    Noti: Observable<VNoti>+'static+Send+Sync+Clone,
    Src : Observable<V>,
    Self: Sized
{
    fn take_until(self, noti:  Noti) -> TakeUntilOp<VNoti, Src, Noti>;
}

impl<V, Src, VNoti, Noti> ObservableTakeUntil<V, Src, VNoti, Noti> for Src where
    Noti: Observable<VNoti>+'static+Send+Sync+Clone,
    Src : Observable<V>
{
    fn take_until(self, noti: Noti) -> TakeUntilOp<VNoti, Src, Noti>
    {
        TakeUntilOp{ source: self, noti: noti, PhantomData }
    }
}

impl<V:'static+Send+Sync, Src, VNoti, Noti> Observable<V> for TakeUntilOp<VNoti, Src, Noti> where
    Noti: Observable<VNoti>+'static+Send+Sync+Clone,
    Src : Observable<V>
{
    fn sub(&self, dest: Arc<Observer<V>+Send+Sync>) -> UnsubRef<'static>
    {
        let notified = Arc::new(AtomicBool::new(false));
        let notified2 = notified.clone();
        let notified3 = notified.clone();

        let s = Arc::new(Subscriber::new(TakeUntilState{ notified: notified.clone() }, dest, false));

        let noti_sub = self.noti.sub(Arc::new((
            move |v:VNoti| notified2.store(true, Ordering::SeqCst),
            move |e:Arc<Any+Send+Sync>| notified3.store(true, Ordering::SeqCst),
            move || {} //dont notify on complete. (same as rxjs)
        )));

        if notified.load(Ordering::SeqCst) {
            noti_sub.unsub();
            return UnsubRef::empty();
        }

        let sub = self.source.sub(s.clone());
        s.set_unsub(&sub);

        sub
    }
}

impl<V> SubscriberImpl<V, TakeUntilState> for Subscriber<V, TakeUntilState>
{
    fn on_next(&self, v: V)
    {
        if self._state.notified.load(Ordering::SeqCst) {
            self.complete();
            return;
        }
        self._dest.next(v);
    }

    fn on_err(&self, e: Arc<Any+Send+Sync>)
    {
        self.do_unsub();
        self._dest.err(e);
    }

    fn on_comp(&self)
    {
        self.do_unsub();
        self._dest.complete();
    }
}

#[cfg(test)]
mod test
{
    use super::*;
    use subject::*;
    use fac::*;
    use std::sync::atomic::AtomicIsize;
    use observable::*;
    use scheduler::NewThreadScheduler;

    #[test]
    fn basic()
    {
        let noti = Arc::new(Subject::<i32>::new());
        let subj = Subject::<i32>::new();

        let mut r = 0;
        {
            let it = subj.rx().take_until(noti.clone()).sub_scoped(|v| r+= 1);
            subj.next(1);

            noti.next(1);

            subj.next(1);
        }

        assert_eq!(r, 1);
    }

    #[test]
    fn threads()
    {
        let noti = Arc::new(Subject::<i32>::new());
        let subj = Subject::<i32>::new();

        let noti2 = noti.clone();

        let mut r = AtomicIsize::new(0);
        {
            let it = subj.rx().take_until(noti.clone()).sub_scoped(|v| { r.fetch_add(1, Ordering::SeqCst);});
            subj.next(1);

            let hr = ::std::thread::spawn(move || noti2.next(1));
            hr.join();

            subj.next(1);
        }

        assert_eq!(r.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn timer()
    {
        let result = Arc::new(AtomicIsize::new(0));
        let (r1, r2) = (result.clone(), result.clone());

        let subj = Subject::new();
        subj.rx().take_until(rxfac::timer(100, None, NewThreadScheduler::get())).subf(move |v| {r1.store(v, Ordering::SeqCst);}, (), move || {r2.store(100, Ordering::SeqCst);});
        subj.next(1);
        assert_eq!(result.load(Ordering::SeqCst), 1);

        ::std::thread::sleep(::std::time::Duration::from_secs(1));
        subj.next(2);

        assert_eq!(result.load(Ordering::SeqCst), 100);
    }
}