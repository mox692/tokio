//! Snapshots of runtime state.
//!
//! See [Handle::dump][crate::runtime::Handle::dump].

use crate::runtime::task::Symbol as SymbolInner;
use crate::runtime::task::Tree as TreeInner;
use crate::task::Id;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

/// A snapshot of a runtime's state.
///
/// See [Handle::dump][crate::runtime::Handle::dump].
#[derive(Debug)]
pub struct Dump {
    tasks: Tasks,
}

/// Snapshots of tasks.
///
/// See [Handle::dump][crate::runtime::Handle::dump].
#[derive(Debug)]
pub struct Tasks {
    tasks: Vec<Task>,
}

/// A snapshot of a task.
///
/// See [Handle::dump][crate::runtime::Handle::dump].
#[derive(Debug)]
pub struct Task {
    id: Id,
    trace: Trace,
}

impl Task {
    /// Returns a trace tree of this task.
    pub fn task_trace(&self) -> Tree {
        let inner = self.trace.inner.trace_tree();
        Tree::from(inner)
    }
}

/// docs
#[derive(Debug)]
pub struct Tree {
    /// The roots of the trees.
    ///
    /// There should only be one root, but the code is robust to multiple roots.
    pub roots: HashSet<Symbol>,

    /// The adjacency list of symbols in the execution tree(s).
    pub edges: HashMap<Symbol, HashSet<Symbol>>,
}

/// docs
#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub struct Symbol {
    /// docs
    pub name: Option<Vec<u8>>,
    /// docs
    pub addr: Option<usize>,
    /// docs
    pub filename: Option<PathBuf>,
    /// docs
    pub lineno: Option<u32>,
    /// docs
    pub colno: Option<u32>,
}

impl From<SymbolInner> for Symbol {
    fn from(symbol: SymbolInner) -> Self {
        Self {
            name: symbol.symbol.name().map(|s| s.as_bytes().to_vec()),
            addr: symbol.symbol.addr().map(|a| a as usize),
            filename: symbol.symbol.filename().map(PathBuf::from),
            lineno: symbol.symbol.lineno(),
            colno: symbol.symbol.colno(),
        }
    }
}

impl From<TreeInner> for Tree {
    fn from(tree: TreeInner) -> Self {
        Self {
            roots: tree.roots.into_iter().map(Symbol::from).collect(),
            edges: tree
                .edges
                .into_iter()
                .map(|(k, v)| (Symbol::from(k), v.into_iter().map(Symbol::from).collect()))
                .collect(),
        }
    }
}

/// An execution trace of a task's last poll.
///
/// See [Handle::dump][crate::runtime::Handle::dump].
#[derive(Debug)]
pub struct Trace {
    inner: super::task::trace::Trace,
}

impl Dump {
    pub(crate) fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: Tasks { tasks },
        }
    }

    /// Tasks in this snapshot.
    pub fn tasks(&self) -> &Tasks {
        &self.tasks
    }
}

impl Tasks {
    /// Iterate over tasks.
    pub fn iter(&self) -> impl Iterator<Item = &Task> {
        self.tasks.iter()
    }
}

impl Task {
    pub(crate) fn new(id: Id, trace: super::task::trace::Trace) -> Self {
        Self {
            id,
            trace: Trace { inner: trace },
        }
    }

    /// Returns a [task ID] that uniquely identifies this task relative to other
    /// tasks spawned at the time of the dump.
    ///
    /// **Note**: This is an [unstable API][unstable]. The public API of this type
    /// may break in 1.x releases. See [the documentation on unstable
    /// features][unstable] for details.
    ///
    /// [task ID]: crate::task::Id
    /// [unstable]: crate#unstable-features
    #[cfg(tokio_unstable)]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn id(&self) -> Id {
        self.id
    }

    /// A trace of this task's state.
    pub fn trace(&self) -> &Trace {
        &self.trace
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
