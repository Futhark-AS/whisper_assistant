class JavaCodeTextSplitter():
    def __init__(self, max_lines_per_chunk: int = 50, extra_lines_before_split:int = 0):
        self.max_lines_per_chunk = max_lines_per_chunk
        self.extra_lines_before_split = extra_lines_before_split
        self.method_start_patterns = [
            "public ",
            "private ",
            "protected "
        ]

    def split_text(self, text: str) -> list[str]:
        lines = text.split('\n')
        chunks = []
        current_chunk = []
        line_count = 0
        in_method = False
        method_nesting_level = 0

        for index, line in enumerate(lines):
            line_count += 1
            if any(line.strip().startswith(pattern) for pattern in self.method_start_patterns) and ('{' in line or lines[index+1].strip() == '{'):
                if not in_method:
                    if line_count > 1:
                        start_index = max(index - self.extra_lines_before_split, 0)
                        lines_to_include = [l for l in lines[start_index:index] if l.strip().startswith('//')]
                        chunks.append('\n'.join(lines_to_include + current_chunk))
                        current_chunk = []
                        line_count = 1
                    in_method = True
                method_nesting_level += 1

            current_chunk.append(line)

            if in_method and '}' in line:
                method_nesting_level -= 1
                if method_nesting_level == 0:
                    in_method = False

            if not in_method and line_count == self.max_lines_per_chunk:
                chunks.append('\n'.join(current_chunk))
                current_chunk = []
                line_count = 0

            if in_method and len(current_chunk) > self.max_lines_per_chunk:
                raise ValueError(f'Method too large for max_lines_per_chunk: {self.max_lines_per_chunk}')

        if current_chunk:
            chunks.append('\n'.join(current_chunk))

        return chunks