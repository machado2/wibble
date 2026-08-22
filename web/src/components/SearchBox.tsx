import React, { useEffect, useState } from "react";
import { Input } from "antd";
import { useDebounce } from "use-debounce";
import { useRouter } from "next/router";
import { useGlobalLanguage } from "./GlobalLanguage";

const SearchBox = () => {
  const router = useRouter();
  const { copy } = useGlobalLanguage();
  const searchTerm = (router.query.search as string) || undefined;
  const [inputValue, setInputValue] = useState(searchTerm);
  const [debouncedValue] = useDebounce(inputValue, 800);

  useEffect(() => {
    setInputValue(searchTerm);
  }, [searchTerm]);

  useEffect(() => {
    if (!router.isReady) return;
    const newSearchParameter =
      (debouncedValue?.length ?? 0) > 0 ? debouncedValue : undefined;
    if (newSearchParameter === searchTerm) return;
    const query: any = { ...router.query };
    delete query.afterId;
    if (!newSearchParameter) {
      delete query.search;
    } else {
      query.search = newSearchParameter;
    }
    void router.push({
      pathname: router.pathname,
      query,
    });
  }, [debouncedValue, router, searchTerm]);

  const handleSearchChange: React.ChangeEventHandler<HTMLInputElement> = (
    e
  ) => {
    const newValue = e.target.value;
    if (newValue !== inputValue) {
      setInputValue(newValue);
    }
  };


  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      setInputValue("");
    }
  };

  return (
    <Input
      placeholder={copy.searchPlaceholder}
      value={inputValue}
      onChange={handleSearchChange}
      onKeyDown={handleKeyDown}
      style={{ marginBottom: "20px", marginTop: "20px" }}
      autoFocus
      allowClear
    />
  );
};

export default SearchBox;
