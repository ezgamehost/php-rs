use super::*;

// Trial deletion over object graphs. References held outside this graph are
// roots, including runtime temporaries and embedding values. Arrays and cells
// are separate vertices so shared containers do not multiply their edges.
struct Node {
    value: Value,
    incoming: usize,
    edges: Vec<usize>,
}

fn identity(value: &Value) -> Option<(u8, usize)> {
    match value {
        Value::Object(value) => Some((0, Rc::as_ptr(value) as usize)),
        Value::Array(value) => Some((1, Rc::as_ptr(value) as usize)),
        Value::Reference(value) | Value::Indirect(value) => Some((2, Rc::as_ptr(value) as usize)),
        _ => None,
    }
}

fn add_node(
    value: Value,
    nodes: &mut Vec<Node>,
    indices: &mut HashMap<(u8, usize), usize>,
) -> Option<usize> {
    let key = identity(&value)?;
    Some(*indices.entry(key).or_insert_with(|| {
        let index = nodes.len();
        nodes.push(Node {
            value,
            incoming: 0,
            edges: Vec::new(),
        });
        index
    }))
}

impl Runtime {
    fn cycle_graph(&self) -> (Vec<Node>, Vec<bool>) {
        let mut nodes = Vec::new();
        let mut indices = HashMap::new();
        for object in self
            .object_store
            .borrow()
            .objects
            .values()
            .filter_map(std::rc::Weak::upgrade)
        {
            add_node(Value::Object(object), &mut nodes, &mut indices);
        }
        let mut index = 0;
        while index < nodes.len() {
            let children: Vec<Value> = match &nodes[index].value {
                Value::Object(object) => {
                    let mut values: Vec<_> = object
                        .properties
                        .borrow()
                        .entries()
                        .iter()
                        .map(|(_, value)| value.clone())
                        .collect();
                    if let Some(data) = object.array_iterator.borrow().as_ref() {
                        values.push(Value::Indirect(data.storage.clone()));
                    }
                    values
                }
                Value::Array(array) => array
                    .entries()
                    .iter()
                    .map(|(_, value)| value.clone())
                    .collect(),
                Value::Reference(cell) | Value::Indirect(cell) => vec![cell.borrow().clone()],
                _ => unreachable!(),
            };
            for value in children {
                if let Some(child) = add_node(value, &mut nodes, &mut indices) {
                    nodes[child].incoming += 1;
                    nodes[index].edges.push(child);
                }
            }
            index += 1;
        }
        let mut reachable = vec![false; nodes.len()];
        let mut pending = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let count = match &node.value {
                Value::Object(value) => Rc::strong_count(value),
                Value::Array(value) => Rc::strong_count(value),
                Value::Reference(value) | Value::Indirect(value) => Rc::strong_count(value),
                _ => unreachable!(),
            };
            if count > node.incoming + 1 {
                reachable[index] = true;
                pending.push(index);
            }
        }
        while let Some(index) = pending.pop() {
            for &child in &nodes[index].edges {
                if !reachable[child] {
                    reachable[child] = true;
                    pending.push(child);
                }
            }
        }
        (nodes, reachable)
    }

    pub(super) fn collect_cycles(&mut self, line: usize) -> Result<Value, Error> {
        if self.collecting_cycles {
            return Ok(Value::Int(0));
        }
        self.collecting_cycles = true;
        let result = self.collect_object_cycles(line);
        self.collecting_cycles = false;
        result.map(Value::Int)
    }

    fn collect_object_cycles(&mut self, line: usize) -> Result<i64, Error> {
        let (nodes, reachable) = self.cycle_graph();
        // Destructors run before breaking edges, and can resurrect any member
        // of a cycle. Recompute reachability after releasing this snapshot.
        for (node, reachable) in nodes.iter().zip(&reachable) {
            if !reachable
                && let Value::Object(object) = &node.value
                && !object._handle.destructor_called.get()
            {
                self.invoke_destructor(object, line)?;
            }
        }
        drop(nodes);
        self.drain_destructors(Ok(()), line)?;
        let (nodes, reachable) = self.cycle_graph();
        let mut count = 0;
        for (node, reachable) in nodes.iter().zip(&reachable) {
            if *reachable {
                continue;
            }
            match &node.value {
                Value::Object(object) => {
                    count += 1;
                    let properties = std::mem::take(&mut *object.properties.borrow_mut());
                    drop(properties);
                    let native = object.array_iterator.borrow_mut().take();
                    drop(native);
                }
                Value::Array(_) => count += 1,
                Value::Reference(cell) | Value::Indirect(cell) => {
                    let value = std::mem::replace(&mut *cell.borrow_mut(), Value::Null);
                    drop(value);
                }
                _ => unreachable!(),
            }
        }
        drop(nodes);
        self.drain_destructors(Ok(count), line)
    }
}
