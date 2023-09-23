use thin_vec::ThinVec;

use crate::{Array, Repr, Vekk};

pub struct Iter<A: Array>(IterRepr<A>);

enum IterRepr<A: Array> {
    Inline(InlineIter<A>),
    Heap(<ThinVec<A::Item> as IntoIterator>::IntoIter),
}

impl<A: Array> Iterator for Iter<A> {
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IterRepr::Inline(iter) => iter.next(),
            IterRepr::Heap(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            IterRepr::Inline(inline) => inline.size_hint(),
            IterRepr::Heap(heap) => heap.size_hint(),
        }
    }
}

struct InlineIter<A: Array> {
    pos: u16,
    len: u16,
    array: A,
}

impl<A: Array> Iterator for InlineIter<A> {
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.len {
            None
        } else {
            let item = core::mem::take(&mut self.array.as_slice_mut()[self.pos as usize]);
            self.pos += 1;
            Some(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.len - self.pos) as usize;
        (remaining, Some(remaining))
    }
}

impl<A: Array> IntoIterator for Vekk<A> {
    type Item = A::Item;
    type IntoIter = Iter<A>;

    fn into_iter(self) -> Self::IntoIter {
        match self.repr {
            Repr::Inline { len, array } => {
                Iter(IterRepr::Inline(InlineIter { pos: 0, len, array }))
            }
            Repr::Heap(vec) => Iter(IterRepr::Heap(vec.into_iter())),
        }
    }
}
