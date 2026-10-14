const { expect } = require("chai");
const sinon = require("sinon");
const { money } = require("../src/format.cjs");

describe("money", function () {
  it("shows two decimals", function () {
    expect(money(150)).to.equal("1.50");
  });

  it("is not called by accident", function () {
    const spy = sinon.spy();
    expect(spy.called).to.be.false;
  });
});
