import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { openExternal } from "../lib/openExternal";

// Raw HTML in the text is never rendered (react-markdown's default), only markdown.
const components: Components = {
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      onClick={(e) => {
        if (!href) return;
        e.preventDefault();
        openExternal(href);
      }}
    >
      {children}
    </a>
  ),
};

/** Assistant text with GitHub-flavoured markdown (bold, lists, code, tables, links). */
export default function Markdown({ text }: { text: string }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
      {text}
    </ReactMarkdown>
  );
}
