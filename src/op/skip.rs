use std::marker::PhantomData;
use std::any::{Any};
use std::rc::Rc;
use std::sync::atomic::AtomicIsize;

use observable::*;
use subscriber::*;
use subref::SubRef;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub struct SkipOp<Src, V>
{
    source: Src,
    total: isize,
    PhantomData: PhantomData<V>
}

struct SkipState
{
    count:AtomicIsize
}

pub trait ObservableSkip<'a, Src, V> where Src : Observable<'a,V>
{
    fn skip(self, total: isize) -> SkipOp<Src, V>;
}

impl<'a,Src, V> ObservableSkip<'a, Src, V> for Src where Src : Observable<'a, V>,
{
    fn skip(self, total: isize) -> SkipOp<Self, V>
    {
        SkipOp{ total, PhantomData, source: self  }
    }
}

impl<'a, V,Dest> SubscriberImpl<V,SkipState> for Subscriber<'a, V,SkipState,Dest> where Dest: Observer<V>+Send+Sync+'a
{
    fn on_next(&self, v:V)
    {
        if self._state.count.load(Ordering::Acquire) <= 0 {
            if self._dest._is_closed() {
                self.complete();
                return;
            }
            self._dest.next(v);
            if self._dest._is_closed() {
                self.complete();
            }
            return;
        }

        self._state.count.fetch_sub(1, Ordering::SeqCst);
    }

    fn on_err(&self, e:Arc<Any+Send+Sync>)
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

impl<'a, Src, V:'static+Send+Sync> Observable<'a,V> for SkipOp<Src, V> where Src: Observable<'a, V>
{
    fn sub(&self, dest: impl Observer<V> + Send + Sync+'a) -> SubRef
    {
        let s = Subscriber::new(SkipState{ count: AtomicIsize::new(self.total)}, dest, false);
        s.do_sub(&self.source)
    }
}

#[cfg(test)]
mod test
{
    use super::*;
    use subject::*;
    use observable::RxNoti::*;

    #[test]
    fn basic()
    {
        let mut result = 0;
        {
            let s = Subject::new();

            s.rx().skip(1).sub_noti(|n| match n {
               Next(v) => result += v ,
               Comp => result += 100,
                _=> {}
            });
            s.next(1);
            s.next(2);
            s.complete();
        }

        assert_eq!(result, 102);
    }
}