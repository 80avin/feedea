import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { Input } from "@heroui/react";
import { encodeId } from "../api/client";
import { useDebounce } from "../hooks/useDebounce";
import { useSuggestions } from "../state/hooks";
import SearchSuggestions from "./SearchSuggestions";

export default function SearchBar() {
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const currentQ = searchParams.get("q") ?? "";
  const [value, setValue] = useState(currentQ);
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const debounced = useDebounce(value, 300);
  const { suggestions } = useSuggestions(debounced);

  useEffect(() => {
    setValue(currentQ);
    setOpen(false);
  }, [currentQ]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const applyFilter = (term: string) => {
    const next = new URLSearchParams(searchParams);
    const trimmed = term.trim();
    if (trimmed) {
      next.set("q", trimmed);
    } else {
      next.delete("q");
    }
    setSearchParams(next, { replace: true });
    setOpen(false);
  };

  const showDropdown = open && debounced.trim().length > 0 && suggestions.length > 0;

  return (
    <div ref={containerRef} className="relative">
      <Input
        aria-label="Search articles"
        type="search"
        placeholder="Search articles…"
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            applyFilter(value);
            (e.target as HTMLInputElement).blur();
          } else if (e.key === "Escape") {
            setOpen(false);
          }
        }}
      />
      {showDropdown && (
        <SearchSuggestions
          suggestions={suggestions}
          onSelect={(id) => {
            setOpen(false);
            navigate(`/feeds/${encodeId(id)}`);
          }}
        />
      )}
    </div>
  );
}
