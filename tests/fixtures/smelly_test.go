package smelly_test

import (
	"fmt"
	"testing"
	"time"
)

// Empty Test — テストが空っぽ
func TestEmpty(t *testing.T) {
}

// Missing Assertion — アサーションがない
func TestNoAssertion(t *testing.T) {
	result := Add(2, 3)
	_ = result
}

// Sleepy Test — time.Sleep を使っている
func TestSleepy(t *testing.T) {
	time.Sleep(3 * time.Second)
	if got := Add(1, 2); got != 3 {
		t.Errorf("expected 3, got %d", got)
	}
}

// Conditional Test Logic — テスト内の条件分岐
func TestConditional(t *testing.T) {
	if runtime.GOOS == "linux" {
		t.Error("linux specific failure")
	} else {
		t.Log("skipping on non-linux")
	}
}

// Ignored Test — t.Skip で無視
func TestSkipped(t *testing.T) {
	t.Skip("not implemented yet")
	if 1 != 1 {
		t.Fatal("impossible")
	}
}

// Redundant Print — デバッグ出力の残骸
func TestRedundantPrint(t *testing.T) {
	result := Add(2, 3)
	fmt.Println("debug result:", result)
	t.Log("logging for debug")
	if result != 5 {
		t.Error("wrong result")
	}
}

// Magic Number — マジックナンバー
func TestMagicNumber(t *testing.T) {
	if Add(100, 200) != 300 {
		t.Error("wrong")
	}
}

// Giant Test — 長すぎるテスト
func TestGiant(t *testing.T) {
	a := 1
	b := 2
	c := 3
	d := 4
	e := 5
	f := 6
	g := 7
	h := 8
	i := 9
	j := 10
	k := 11
	l := 12
	m := 13
	n := 14
	o := 15
	p := 16
	q := 17
	r := 18
	s := 19
	tt2 := 20
	u := 21
	v := 22
	w := 23
	x := 24
	y := 25
	z := 26
	aa := 27
	bb := 28
	cc := 29
	dd := 30
	if a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+tt2+u+v+w+x+y+z+aa+bb+cc+dd != 465 {
		t.Error("wrong sum")
	}
}

// テーブル駆動テスト（これ自体は良いパターン）
func TestTableDriven(t *testing.T) {
	tests := []struct {
		a, b, want int
	}{
		{1, 2, 3},
		{0, 0, 0},
		{-1, 1, 0},
	}
	for _, tt := range tests {
		if got := Add(tt.a, tt.b); got != tt.want {
			t.Errorf("Add(%d, %d) = %d, want %d", tt.a, tt.b, got, tt.want)
		}
	}
}
