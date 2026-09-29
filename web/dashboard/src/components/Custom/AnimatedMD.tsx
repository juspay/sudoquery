import { useEffect, useRef, useState } from 'react';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import {encode, decode} from 'gpt-tokenizer';

const testMd = `# The Complete Guide to Everything

## Table of Contents
1. [Introduction](#introduction)
2. [History](#history)
3. [Science](#science)
4. [Code](#code)
5. [Philosophy](#philosophy)

---

## Introduction

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.

> "The only way to do great work is to love what you do."
> — Someone famous

This document covers **everything you need to know** about _absolutely nothing_ in particular. It's great for testing your markdown renderer because it includes:

- Headings (h1–h4)
- **Bold**, _italic_, ~~strikethrough~~, and \`inline code\`
- Nested lists
- Blockquotes
- Tables
- Code blocks
- Horizontal rules
- Links and images

---

## History

### Ancient Times

Way back when, people used to write on **stone tablets**. This was inconvenient for several reasons:

1. Heavy to carry
2. Hard to edit
   - No undo button
   - Chisels are expensive
3. Poor battery life

### The Middle Ages

Then came the _printing press_, which changed everything. Key facts:

- Invented by **Johannes Gutenberg** around 1440
- Enabled mass production of books
- Led to the **Renaissance**, the **Reformation**, and arguably the internet

### Modern Era

Now we have markdown, which is:

> A lightweight markup language that you can use to add formatting elements to plaintext text documents.

Pretty neat.

---

## Science

### Physics

The famous equation by Einstein:

**E = mc²**

Where:
- \`E\` = energy
- \`m\` = mass
- \`c\` = speed of light (~300,000 km/s)

### Biology

Cells are the basic unit of life. There are two types:

1. **Prokaryotic** — no nucleus
   - Bacteria
   - Archaea
2. **Eukaryotic** — has a nucleus
   - Animals
   - Plants
   - Fungi
   - Protists

### Chemistry

The periodic table has **118 elements**. Some fun ones:

| Element | Symbol | Atomic Number | Fun Fact |
|---------|--------|---------------|----------|
| Gold | Au | 79 | Doesn't tarnish |
| Mercury | Hg | 80 | Liquid at room temp |
| Carbon | C | 6 | Basis of all life |
| Oxygen | O | 8 | You need this |
| Francium | Fr | 87 | Extremely rare |

---

## Code

### JavaScript

Here's a classic recursive function:

\`\`\`js
function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

console.log(fibonacci(10)); // 55
\`\`\`

### Python

Same thing in Python:

\`\`\`python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

print(fibonacci(10))  # 55
\`\`\`

### A More Complex Example

\`\`\`ts
type Tree<T> = {
  value: T;
  children?: Tree<T>[];
};

function walk<T>(node: Tree<T>, visitor: (val: T) => void): void {
  visitor(node.value);
  node.children?.forEach(child => walk(child, visitor));
}

const tree: Tree<string> = {
  value: 'root',
  children: [
    { value: 'a', children: [{ value: 'a1' }, { value: 'a2' }] },
    { value: 'b' },
  ],
};

walk(tree, console.log);
// root, a, a1, a2, b
\`\`\`

---

## Philosophy

### The Big Questions

People have wondered for millennia:

1. Why are we here?
2. What is consciousness?
3. Is hot dog a sandwich?

### Nested Blockquotes

> Socrates once said something wise.
>
> > Plato wrote it down, probably paraphrasing.
> >
> > > Aristotle disagreed with both of them.

### Inline Everything

This sentence has **bold**, _italic_, **_bold italic_**, ~~strikethrough~~, \`inline code\`, and [a link](https://example.com) all at once. It also has a footnote-style aside — which is useful for **testing how your renderer handles ~mixed~ inline nodes** back to back without breaking.

---

## Miscellaneous

### Task List

- [x] Write markdown test document
- [x] Include nested lists
- [ ] Add more obscure edge cases
- [ ] Ship to production
- [ ] Sleep

### Deeply Nested List

- Level 1
  - Level 2
    - Level 3
      - Level 4
        - Level 5 (okay this is getting ridiculous)

### Short Paragraphs

One.

Two.

Three.

### Long Unbroken Word Test

\`supercalifragilisticexpialidocious_and_then_some_more_text_that_keeps_going\`

### Final Thoughts

That's all. If your renderer handled all of that without exploding — **nice work**. 🎉`

const tokens = encode(testMd);

function AnimatedMDTest(){
  const [t, setT] = useState(1);
  useEffect(()=>{
    if(t >= tokens.length) return;
    const interval = setInterval(()=>{setT((t)=>t+30)}, 300);
    return ()=>{clearInterval(interval)};
  }, [t == 100]);
  return <div>
    <AnimatedMD md={decode(tokens.slice(0, Math.min(t, tokens.length)))}/>
  </div>
}

export function AnimatedMD({md, trimLeadingBlockMargin = false}: {md: string; trimLeadingBlockMargin?: boolean}){
  return <MDUI ast={parseMD(md)} trimLeadingBlockMargin={trimLeadingBlockMargin}/>
}

interface Ast {
  type: string;
  children: Ast[];
  value: string;
  ordered: boolean;
  url: string;
}

function MDUI({
  ast,
  isHeaderCell = false,
  isLastChild = false,
  rowIndex = 0,
  trimLeadingBlockMargin = false,
  isFirstChild = false,
}: {
  ast: Ast;
  isHeaderCell?: boolean;
  isLastChild?: boolean;
  rowIndex?: number;
  trimLeadingBlockMargin?: boolean;
  isFirstChild?: boolean;
}){
  switch (ast.type) {
    case "root":
      return <div className='new-block'>
          {ast.children.map((a, index) => MDUI({ast:a, trimLeadingBlockMargin, isFirstChild: index === 0}))}
        </div>
    case "break":
      return <br/>
    case "text":
      return <AnimatedSpan value={ast.value}/>
    case "heading":
      return <h3 style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </h3>
    case "list":
      if(ast.ordered){
        return <ol className='new-block' style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
            {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
        </ol>
      }
      return <ul className='new-block' style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </ul>
    case "listItem":
      return <li>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </li>
    case "paragraph":
      return <p style={{marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : '0.8rem', marginBlockEnd: '0.8rem'}}>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </p>
    case "link":
      return <a href={ast.url}>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </a>
    case "thematicBreak":
      return <hr style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }} />
    case "blockquote":
      return <blockquote style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </blockquote>
    case "strong":
      return <strong>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </strong>
    case "emphasis":
      return <em>
          {ast.children.map(a => MDUI({ast:a, trimLeadingBlockMargin}))}
      </em>
    case "inlineCode":
      return <code>
          {ast.value}
      </code>
    case "code":
      return <pre style={{ marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
        <code>
          {ast.value}
        </code>
      </pre>
    case "table":
      const headerRow = ast.children[0];
      const bodyRows = ast.children.slice(1);
      return <table style={{ borderCollapse: 'collapse', width: '100%', marginBlockStart: trimLeadingBlockMargin && isFirstChild ? '0' : undefined }}>
        <thead>{MDUI({ ast: headerRow, isHeaderCell: true, rowIndex: 0, trimLeadingBlockMargin })}</thead>
        <tbody>
          {bodyRows.map((a, i) => MDUI({ ast: a, rowIndex: i + 1, trimLeadingBlockMargin }))}
        </tbody>
      </table>
    case "tableRow":
      const bgColor = rowIndex % 2 === 0 ? 'transparent' : 'rgba(0, 0, 0, 0.05)';
      return <tr style={{ backgroundColor: bgColor }}>
        {ast.children.map(a => MDUI({ ast: a, isHeaderCell }))}
      </tr>
    case "tableCell":
      const cellStyle = { border: '1px solid #ccc', padding: '8px 12px' };
      if (isHeaderCell) {
        return <th style={{ ...cellStyle, backgroundColor: '#f5f5f5', fontWeight: 600, textAlign: 'left' }}>
          {ast.children.map(a => MDUI({ ast: a, trimLeadingBlockMargin }))}
        </th>
      }
      return <td style={cellStyle}>
        {ast.children.map(a => MDUI({ ast: a, trimLeadingBlockMargin }))}
      </td>
    case "delete":
      return <del>
        {ast.children.map(a => MDUI({ ast: a, trimLeadingBlockMargin }))}
      </del>
    case "html":
      return <div dangerouslySetInnerHTML={{ __html: ast.value }} />
  }
  return <p style={{background: "red"}}>something went wrong in message parsing <br/> {JSON.stringify(ast)}</p>
}

function AnimatedSpan({value, isLastChild=true}: {value: string, isLastChild?: boolean}){
  const prevValue = useRef(value)
  useEffect(()=>{
    prevValue.current = value;
  })
  if(isLastChild){
    const newVal = value.substring(prevValue.current.length, value.length);
    return <span><span style={{'white-space': 'pre-wrap'}}>{prevValue.current}</span><span className='new-token' style={{'white-space': 'pre-wrap'}}>{newVal}</span></span>
  } else {
    return <span>{value}</span>
  }
}

function parseMD(markdown: string){
  const ast = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .parse(markdown)

  return ast
}
