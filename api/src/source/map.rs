//! I define [`MapSource`], the result type of [`Source::map_items`].
use super::*;
use std::{collections::VecDeque, error::Error};

/// The result of [`Source::map_items`].
pub struct MapSource<S, F> {
    pub(super) source: S,
    pub(super) map: F,
}

impl<S, F, T> Source for MapSource<S, F>
where
    S: Source,
    F: FnMut(S::Item<'_>) -> T,
{
    type Item<'x> = T;
    type Error = S::Error;

    fn try_for_some_item<E, F2>(&mut self, mut f: F2) -> StreamResult<bool, Self::Error, E>
    where
        E: Error + Send + Sync + 'static,
        F2: FnMut(Self::Item<'_>) -> Result<(), E>,
    {
        let map = &mut self.map;
        self.source.try_for_some_item(|t| f((map)(t)))
    }

    fn size_hint_items(&self) -> (usize, Option<usize>) {
        self.source.size_hint_items()
    }
}

impl<S, F, T> IntoIterator for MapSource<S, F>
where
    S: Source,
    F: FnMut(S::Item<'_>) -> T,
{
    type Item = Result<T, S::Error>;
    type IntoIter = MapSourceIterator<S, F, T, S::Error>;

    fn into_iter(self) -> Self::IntoIter {
        MapSourceIterator {
            source: self.source,
            map: self.map,
            buffer: VecDeque::new(),
        }
    }
}

/// [`Iterator`] implementation for the returned value of [`Source::map_items`].
pub struct MapSourceIterator<S, F, T, E> {
    source: S,
    map: F,
    buffer: VecDeque<Result<T, E>>,
}

impl<S, F, T> Iterator for MapSourceIterator<S, F, T, S::Error>
where
    S: Source,
    F: FnMut(S::Item<'_>) -> T,
{
    type Item = Result<T, S::Error>;
    fn next(&mut self) -> Option<Result<T, S::Error>> {
        let mut remaining = true;
        let mut buffer = VecDeque::new();
        std::mem::swap(&mut self.buffer, &mut buffer);
        while buffer.is_empty() && remaining {
            let resb = self.source.for_some_item(|i| {
                buffer.push_back(Ok((self.map)(i)));
            });
            match resb {
                Ok(b) => {
                    remaining = b;
                }
                Err(err) => {
                    buffer.push_back(Err(err));
                    remaining = false;
                }
            }
        }
        std::mem::swap(&mut self.buffer, &mut buffer);
        self.buffer.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint_items()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::dataset::{Dataset, MutableDataset};
    use crate::graph::{Graph, MutableGraph};
    use crate::quad::{Quad, Spog};
    use crate::term::ez_term;
    use crate::term::{SimpleTerm, Term};
    use crate::triple::Triple;

    // check that the result of TripleSource::map_triples implements the expected traits,
    // and that they work as expected

    #[test]
    fn ts_map_to_triples() {
        let g = vec![
            [ez_term(":a"), ez_term(":b"), ez_term(":c")],
            [ez_term(":d"), ez_term(":e"), ez_term(":f")],
            [ez_term(":g"), ez_term(":h"), ez_term(":i")],
        ];
        let mut h: Vec<[SimpleTerm; 3]> = vec![];
        g.triples()
            .map_triples(|t| [t.o(), t.p(), t.s()])
            .for_each_triple(|t| {
                h.insert_triple(t).unwrap();
            })
            .unwrap();
        assert_eq!(
            h,
            vec![
                [ez_term(":c"), ez_term(":b"), ez_term(":a")],
                [ez_term(":f"), ez_term(":e"), ez_term(":d")],
                [ez_term(":i"), ez_term(":h"), ez_term(":g")],
            ]
        )
    }

    #[test]
    fn ts_map_to_quads() {
        let g = vec![
            [ez_term(":a"), ez_term(":b"), ez_term(":c")],
            [ez_term(":d"), ez_term(":e"), ez_term(":f")],
            [ez_term(":g"), ez_term(":h"), ez_term(":i")],
        ];
        let mut h: Vec<Spog<SimpleTerm>> = vec![];
        g.triples()
            .map_triples(|t| ([t.o(), t.p(), t.s()], None))
            .for_each_quad(|q| {
                h.insert_quad(q).unwrap();
            })
            .unwrap();
        assert_eq!(
            h,
            vec![
                ([ez_term(":c"), ez_term(":b"), ez_term(":a")], None),
                ([ez_term(":f"), ez_term(":e"), ez_term(":d")], None),
                ([ez_term(":i"), ez_term(":h"), ez_term(":g")], None),
            ]
        )
    }

    #[test]
    fn ts_map_iter() {
        let g = vec![
            [ez_term(":a"), ez_term(":b"), ez_term(":c")],
            [ez_term(":d"), ez_term(":e"), ez_term(":f")],
            [ez_term(":g"), ez_term(":h"), ez_term(":i")],
        ];
        let h: Result<Vec<String>, _> = g
            .triples()
            .map_triples(|t| t.s().iri().unwrap().to_string())
            .into_iter()
            .collect();
        assert_eq!(
            h.unwrap(),
            vec![
                "tag:a".to_string(),
                "tag:d".to_string(),
                "tag:g".to_string(),
            ]
        )
    }

    // check that the result of QuadSource::map_quads implements the expected traits
    // and that they work as expected

    #[test]
    fn qs_map_to_triples() {
        let d = vec![
            ([ez_term(":a"), ez_term(":b"), ez_term(":c")], None),
            ([ez_term(":d"), ez_term(":e"), ez_term(":f")], None),
            ([ez_term(":g"), ez_term(":h"), ez_term(":i")], None),
        ];
        let mut h: Vec<[SimpleTerm; 3]> = vec![];
        d.quads()
            .map_quads(|q| [q.o(), q.p(), q.s()])
            .for_each_triple(|t| {
                h.insert_triple(t).unwrap();
            })
            .unwrap();
        assert_eq!(
            h,
            vec![
                [ez_term(":c"), ez_term(":b"), ez_term(":a")],
                [ez_term(":f"), ez_term(":e"), ez_term(":d")],
                [ez_term(":i"), ez_term(":h"), ez_term(":g")],
            ]
        )
    }

    #[test]
    fn qs_map_to_quads() {
        let d = vec![
            ([ez_term(":a"), ez_term(":b"), ez_term(":c")], None),
            ([ez_term(":d"), ez_term(":e"), ez_term(":f")], None),
            ([ez_term(":g"), ez_term(":h"), ez_term(":i")], None),
        ];
        let mut h: Vec<Spog<SimpleTerm>> = vec![];
        d.quads()
            .map_quads(|q| ([q.o(), q.p(), q.s()], q.g()))
            .for_each_quad(|q| {
                h.insert_quad(q).unwrap();
            })
            .unwrap();
        assert_eq!(
            h,
            vec![
                ([ez_term(":c"), ez_term(":b"), ez_term(":a")], None),
                ([ez_term(":f"), ez_term(":e"), ez_term(":d")], None),
                ([ez_term(":i"), ez_term(":h"), ez_term(":g")], None),
            ]
        )
    }

    #[test]
    fn qs_map_iter() {
        let d = vec![
            ([ez_term(":a"), ez_term(":b"), ez_term(":c")], None),
            ([ez_term(":d"), ez_term(":e"), ez_term(":f")], None),
            ([ez_term(":g"), ez_term(":h"), ez_term(":i")], None),
        ];
        let h: Result<Vec<String>, _> = d
            .quads()
            .map_quads(|q| q.s().iri().unwrap().to_string())
            .into_iter()
            .collect();
        assert_eq!(
            h.unwrap(),
            vec![
                "tag:a".to_string(),
                "tag:d".to_string(),
                "tag:g".to_string(),
            ]
        )
    }
}
